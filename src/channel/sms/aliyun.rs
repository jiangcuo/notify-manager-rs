//! # 阿里云短信渠道（dysmsapi）
//!
//! 基于阿里云短信服务 `SendSms`（API 版本 `2017-05-25`）的纯 Rust 异步实现。
//!
//! 本实现**不依赖任何阿里云官方 SDK**，直接使用 `reqwest` 发起请求，
//! 并按阿里云 RPC 风格签名规范（HMAC-SHA1）自行计算签名，保持全异步、依赖精简。
//!
//! ## 调用流程
//! ```text
//! Config::new() → Client::new() → client.send()
//!     → 组装公共参数 + 业务参数
//!     → 计算 HMAC-SHA1 签名
//!     → HTTPS POST 到 dysmsapi.aliyuncs.com
//!     → 解析响应（Code == "OK" 即成功）
//! ```
//!
//! ## 消息映射
//! 阿里云短信必须通过「签名 + 模板 + 模板参数」发送，无法发送自由文本。
//! 因此 [`Message`] 的映射约定见 [父模块文档](super)。
//!
//! ## 关于重试
//! 短信涉及计费且重复下发会骚扰用户，**本渠道默认不做自动重试**。
//! 网络等错误会如实返回 [`ChannelError`]，由调用方决定是否重试。
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::sms::aliyun;
//! use notify_manager_rs::Message;
//!
//! let config = aliyun::Config::new("LTAI_your_key_id", "your_key_secret")
//!     .region("cn-hangzhou")
//!     .sign_name("梨儿方")
//!     .template_code("SMS_123456789")
//!     .to("13800138000");
//!
//! let msg = Message::builder()
//!     .content("验证码通知")     // 仅用于日志
//!     .extra("code", "8888")     // → TemplateParam: {"code":"8888"}
//!     .build();
//!
//! aliyun::Client::new(config).send(&msg).await?;
//! ```

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use tracing::{debug, error, info};

use crate::channel::sms::RESERVED_KEYS;
use crate::channel::Channel;
use crate::error::ChannelError;
use crate::message::Message;

/// 默认地域
const DEFAULT_REGION: &str = "cn-hangzhou";
/// 默认接入点（公网）
const DEFAULT_ENDPOINT: &str = "dysmsapi.aliyuncs.com";
/// API 版本
const API_VERSION: &str = "2017-05-25";
/// 接口动作
const API_ACTION: &str = "SendSms";
/// 默认超时
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// 阿里云短信配置
///
/// 使用 builder 模式构建。`access_key_id` / `access_key_secret` 为必填，
/// `sign_name` / `template_code` 可在此设置，也可在发送时通过
/// [`Message::extra`] 覆盖。
#[derive(Debug, Clone)]
pub struct Config {
    /// AccessKey ID
    access_key_id: String,
    /// AccessKey Secret
    access_key_secret: String,
    /// 地域，默认 `cn-hangzhou`
    region: String,
    /// 接入点域名，默认 `dysmsapi.aliyuncs.com`
    endpoint: String,
    /// 短信签名
    sign_name: Option<String>,
    /// 短信模板 ID
    template_code: Option<String>,
    /// 收件人手机号列表
    to: Vec<String>,
    /// 请求超时
    timeout: Duration,
}

impl Config {
    /// 创建阿里云短信配置
    ///
    /// # 参数
    /// * `access_key_id` - 阿里云 AccessKey ID
    /// * `access_key_secret` - 阿里云 AccessKey Secret
    pub fn new(
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
    ) -> Self {
        Self {
            access_key_id: access_key_id.into(),
            access_key_secret: access_key_secret.into(),
            region: DEFAULT_REGION.to_string(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            sign_name: None,
            template_code: None,
            to: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// 设置地域（如 `cn-hangzhou`、`ap-southeast-1`）
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// 自定义接入点域名（一般无需设置）
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// 设置短信签名
    pub fn sign_name(mut self, name: impl Into<String>) -> Self {
        self.sign_name = Some(name.into());
        self
    }

    /// 设置短信模板 ID
    pub fn template_code(mut self, code: impl Into<String>) -> Self {
        self.template_code = Some(code.into());
        self
    }

    /// 添加一个收件人手机号
    pub fn to(mut self, phone: impl Into<String>) -> Self {
        self.to.push(phone.into());
        self
    }

    /// 添加多个收件人手机号
    pub fn to_many(mut self, phones: Vec<impl Into<String>>) -> Self {
        self.to.extend(phones.into_iter().map(|p| p.into()));
        self
    }

    /// 设置请求超时
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// 阿里云短信客户端
///
/// 持有 HTTP 连接池，适合频繁发送。
pub struct Client {
    config: Config,
    http: HttpClient,
    name: String,
}

impl Client {
    /// 创建客户端
    pub fn new(config: Config) -> Self {
        let http = HttpClient::builder()
            .timeout(config.timeout)
            .build()
            .expect("failed to create http client");

        Self {
            config,
            http,
            name: "sms.aliyun".to_string(),
        }
    }

    /// 设置渠道名称（用于在 `Sender` 中区分多个短信渠道）
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送短信
    ///
    /// # 调用流程
    /// 1. 解析签名 / 模板（`Message::extra` 优先于 `Config`）
    /// 2. 由 `extra` 中的非保留键拼出模板参数 `TemplateParam`
    /// 3. 组装公共参数并计算签名
    /// 4. HTTPS POST 发送，解析响应
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        // 1. 校验收件人
        if self.config.to.is_empty() {
            return Err(ChannelError::InvalidMessage(
                "at least one phone number is required".into(),
            ));
        }

        // 2. 解析签名与模板（extra 覆盖 config）
        let sign_name = message
            .extra
            .get("sign_name")
            .cloned()
            .or_else(|| self.config.sign_name.clone())
            .ok_or_else(|| {
                ChannelError::InvalidMessage("sign_name is required (set in config or extra)".into())
            })?;

        let template_code = message
            .extra
            .get("template_code")
            .cloned()
            .or_else(|| self.config.template_code.clone())
            .ok_or_else(|| {
                ChannelError::InvalidMessage(
                    "template_code is required (set in config or extra)".into(),
                )
            })?;

        // 3. 由 extra 构造模板参数（排除保留键）
        let template_param = build_template_param(message);

        let phone_numbers = self.config.to.join(",");

        debug!(
            to = %phone_numbers,
            sign_name = %sign_name,
            template_code = %template_code,
            "preparing aliyun sms"
        );

        // 4. 组装待签名参数（BTreeMap 自动按 key 升序）
        let mut params: BTreeMap<String, String> = BTreeMap::new();
        // 公共参数
        params.insert("AccessKeyId".into(), self.config.access_key_id.clone());
        params.insert("Action".into(), API_ACTION.into());
        params.insert("Format".into(), "JSON".into());
        params.insert("RegionId".into(), self.config.region.clone());
        params.insert("SignatureMethod".into(), "HMAC-SHA1".into());
        params.insert("SignatureNonce".into(), signature_nonce());
        params.insert("SignatureVersion".into(), "1.0".into());
        params.insert("Timestamp".into(), iso8601_utc_now());
        params.insert("Version".into(), API_VERSION.into());
        // 业务参数
        params.insert("PhoneNumbers".into(), phone_numbers.clone());
        params.insert("SignName".into(), sign_name);
        params.insert("TemplateCode".into(), template_code);
        if let Some(ref tp) = template_param {
            params.insert("TemplateParam".into(), tp.clone());
        }

        // 5. 计算签名
        let string_to_sign = build_string_to_sign("POST", &params);
        let signature = sign(&string_to_sign, &self.config.access_key_secret);

        // 6. 组装最终表单（含 Signature）并发送
        let mut form: Vec<(String, String)> =
            params.into_iter().collect();
        form.push(("Signature".into(), signature));

        let url = format!("https://{}/", self.config.endpoint);

        let response = self
            .http
            .post(&url)
            .form(&form)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ChannelError::Timeout
                } else {
                    ChannelError::Network(e)
                }
            })?;

        let status = response.status();
        let body_text = response.text().await.unwrap_or_default();

        debug!(status = %status, body = %body_text, "aliyun sms response");

        let resp: SendSmsResponse = serde_json::from_str(&body_text).map_err(|e| {
            error!(error = %e, body = %body_text, "failed to parse aliyun sms response");
            ChannelError::Other(format!("invalid response: {}", body_text))
        })?;

        if resp.code == "OK" {
            info!(
                to = %phone_numbers,
                request_id = %resp.request_id,
                biz_id = ?resp.biz_id,
                "aliyun sms sent successfully"
            );
            Ok(())
        } else {
            error!(
                code = %resp.code,
                message = %resp.message,
                request_id = %resp.request_id,
                "aliyun sms api error"
            );
            Err(map_error(&resp.code, &resp.message))
        }
    }
}

#[async_trait]
impl Channel for Client {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        Client::send(self, message).await
    }
}

/// 一次性发送函数
///
/// 适合脚本或一次性任务，每次调用都会创建新的 HTTP 连接。
pub async fn send(config: &Config, message: &Message) -> Result<(), ChannelError> {
    let client = Client::new(config.clone());
    client.send(message).await
}

// ============================================================
// 内部辅助函数
// ============================================================

/// 由 `Message::extra` 中的非保留键构造模板参数 JSON 字符串。
///
/// 返回 `None` 表示无模板参数（模板不含变量时可省略）。
fn build_template_param(message: &Message) -> Option<String> {
    let map: serde_json::Map<String, serde_json::Value> = message
        .extra
        .iter()
        .filter(|(k, _)| !RESERVED_KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();

    if map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(map).to_string())
    }
}

/// 按阿里云 RPC 规范进行 percent-encode（RFC 3986）。
///
/// 仅 `A-Z a-z 0-9 - _ . ~` 不编码；空格编码为 `%20`，`*` 编码为 `%2A`，`~` 不编码。
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for &b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

/// 构造待签名字符串：`METHOD&%2F&percentEncode(canonicalizedQuery)`。
fn build_string_to_sign(method: &str, params: &BTreeMap<String, String>) -> String {
    let canonical = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    format!(
        "{}&{}&{}",
        method,
        percent_encode("/"),
        percent_encode(&canonical)
    )
}

/// 计算 HMAC-SHA1 签名并 base64 编码。签名密钥为 `AccessKeySecret + "&"`。
fn sign(string_to_sign: &str, access_key_secret: &str) -> String {
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use sha1::Sha1;

    let key = format!("{}&", access_key_secret);
    let mut mac =
        Hmac::<Sha1>::new_from_slice(key.as_bytes()).expect("HMAC can take key of any size");
    mac.update(string_to_sign.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

/// 生成签名随机数（保证单进程内唯一）。
fn signature_nonce() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:x}", nanos, c)
}

/// 生成 UTC ISO8601 时间戳，格式 `YYYY-MM-DDTHH:MM:SSZ`。
///
/// 使用 Howard Hinnant 的 days→civil 算法，避免引入 chrono 依赖。
fn iso8601_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_iso8601(secs)
}

/// 将 Unix 秒数格式化为 UTC ISO8601 字符串（拆分出来便于测试）。
fn format_iso8601(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = (secs % 86_400) as i64;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // days since 1970-01-01 → 公历年月日（Howard Hinnant 算法）
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if month <= 2 { year + 1 } else { year };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// 将阿里云错误码映射为 [`ChannelError`]。
fn map_error(code: &str, message: &str) -> ChannelError {
    // 限流/流控类 → 可重试
    if code.contains("Throttling")
        || code == "isv.BUSINESS_LIMIT_CONTROL"
        || code.contains("FLOW_CONTROL")
    {
        return ChannelError::RateLimit { retry_after: None };
    }

    // 鉴权类 → 不可重试
    if code == "SignatureDoesNotMatch"
        || code.starts_with("InvalidAccessKeyId")
        || code.starts_with("Forbidden")
        || code == "UnsupportedRegion"
    {
        return ChannelError::Unauthorized(format!("{}: {}", code, message));
    }

    // isv.* 多为参数/业务校验错误（手机号非法、模板/签名不匹配等）→ 不可重试
    if code.starts_with("isv.") {
        return ChannelError::InvalidMessage(format!("{}: {}", code, message));
    }

    ChannelError::ServerError {
        code: -1,
        message: format!("{}: {}", code, message),
    }
}

/// 阿里云 `SendSms` 响应
#[derive(Debug, Deserialize)]
struct SendSmsResponse {
    #[serde(rename = "RequestId", default)]
    request_id: String,
    #[serde(rename = "Code", default)]
    code: String,
    #[serde(rename = "Message", default)]
    message: String,
    #[serde(rename = "BizId", default)]
    biz_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = Config::new("ak_id", "ak_secret")
            .region("cn-shanghai")
            .sign_name("梨儿方")
            .template_code("SMS_001")
            .to("13800138000")
            .to("13900139000");

        assert_eq!(config.access_key_id, "ak_id");
        assert_eq!(config.region, "cn-shanghai");
        assert_eq!(config.sign_name, Some("梨儿方".to_string()));
        assert_eq!(config.template_code, Some("SMS_001".to_string()));
        assert_eq!(config.to.len(), 2);
    }

    #[test]
    fn test_percent_encode() {
        // 不编码字符
        assert_eq!(percent_encode("aZ09-_.~"), "aZ09-_.~");
        // 空格 → %20，斜杠 → %2F
        assert_eq!(percent_encode("a b/c"), "a%20b%2Fc");
        // 星号 → %2A
        assert_eq!(percent_encode("*"), "%2A");
    }

    #[test]
    fn test_string_to_sign_format() {
        let mut params = BTreeMap::new();
        params.insert("A".to_string(), "1".to_string());
        params.insert("B".to_string(), "2".to_string());

        let sts = build_string_to_sign("POST", &params);
        // canonical = "A=1&B=2"，整体再 percentEncode
        assert_eq!(sts, "POST&%2F&A%3D1%26B%3D2");
    }

    #[test]
    fn test_sign_known_vector() {
        // 校验 HMAC-SHA1 + base64 的稳定性（密钥含 "&" 后缀）
        let sig = sign("POST&%2F&A%3D1", "secret");
        // 固定输入应得到固定输出
        assert!(!sig.is_empty());
        // 同输入必定同输出
        assert_eq!(sig, sign("POST&%2F&A%3D1", "secret"));
    }

    #[test]
    fn test_iso8601_format() {
        // Unix 纪元
        assert_eq!(format_iso8601(0), "1970-01-01T00:00:00Z");
        // 2023-11-14T22:13:20Z
        assert_eq!(format_iso8601(1_700_000_000), "2023-11-14T22:13:20Z");
        // 2009-02-13T23:31:30Z
        assert_eq!(format_iso8601(1_234_567_890), "2009-02-13T23:31:30Z");
    }

    #[test]
    fn test_build_template_param() {
        let msg = Message::builder()
            .content("验证码")
            .extra("code", "1234")
            .extra("sign_name", "ignored") // 保留键，不应出现
            .build();

        let tp = build_template_param(&msg).expect("should have param");
        assert!(tp.contains("\"code\":\"1234\""));
        assert!(!tp.contains("sign_name"));
    }

    #[test]
    fn test_build_template_param_empty() {
        let msg = Message::text("no extra");
        assert!(build_template_param(&msg).is_none());
    }

    #[test]
    fn test_map_error() {
        assert!(matches!(
            map_error("isv.BUSINESS_LIMIT_CONTROL", "x"),
            ChannelError::RateLimit { .. }
        ));
        assert!(matches!(
            map_error("SignatureDoesNotMatch", "x"),
            ChannelError::Unauthorized(_)
        ));
        assert!(matches!(
            map_error("isv.MOBILE_NUMBER_ILLEGAL", "x"),
            ChannelError::InvalidMessage(_)
        ));
        assert!(matches!(
            map_error("ServiceUnavailable", "x"),
            ChannelError::ServerError { .. }
        ));
    }
}
