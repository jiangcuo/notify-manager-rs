//! # 钉钉渠道模块
//!
//! 提供钉钉机器人 Webhook 消息推送功能。
//!
//! ## 功能特性
//! - 支持 Text / Markdown 消息类型
//! - 支持签名验证（安全设置）
//! - 支持 @ 指定人员
//! - 自动重试机制
//! - 超时控制
//!
//! ## 调用流程
//! ```text
//! Config::new() → Client::new() → client.send() → HTTP POST → 解析响应
//!                                       ↓
//!                              失败时触发重试逻辑
//! ```
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::dingtalk;
//! use notify_manager_rs::Message;
//!
//! // Layer 1: 一次性发送
//! let config = dingtalk::Config::new("webhook_url");
//! dingtalk::send(&config, &Message::text("告警")).await?;
//!
//! // Layer 2: 复用连接
//! let client = dingtalk::Client::new(
//!     dingtalk::Config::new("webhook_url").secret("SEC...")
//! );
//! client.send(&Message::text("告警")).await?;
//! ```

mod message;

pub use message::*;

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::channel::Channel;
use crate::error::ChannelError;
use crate::message::Message;

/// 默认超时时间
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// 默认重试次数
const DEFAULT_RETRY_ATTEMPTS: u32 = 3;

/// 默认重试基础延迟
const DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(200);

/// 默认最大重试延迟
const DEFAULT_MAX_RETRY_DELAY: Duration = Duration::from_secs(5);

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 基础延迟时间
    pub base_delay: Duration,
    /// 最大延迟时间
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_RETRY_ATTEMPTS,
            base_delay: DEFAULT_RETRY_DELAY,
            max_delay: DEFAULT_MAX_RETRY_DELAY,
        }
    }
}

/// 钉钉渠道配置
///
/// 用于配置 Webhook URL、签名密钥、超时和重试策略。
#[derive(Debug, Clone)]
pub struct Config {
    /// Webhook URL
    webhook_url: String,
    /// 签名密钥（可选）
    secret: Option<String>,
    /// 请求超时时间
    timeout: Duration,
    /// 重试配置
    retry: Option<RetryConfig>,
}

impl Config {
    /// 创建钉钉配置
    ///
    /// # 参数
    /// * `webhook_url` - 钉钉机器人 Webhook URL
    ///
    /// # 示例
    /// ```rust
    /// use notify_manager_rs::dingtalk::Config;
    ///
    /// let config = Config::new("https://oapi.dingtalk.com/robot/send?access_token=xxx");
    /// ```
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            secret: None,
            timeout: DEFAULT_TIMEOUT,
            retry: Some(RetryConfig::default()),
        }
    }

    /// 设置签名密钥
    ///
    /// 钉钉机器人安全设置中的「加签」密钥。
    ///
    /// # 参数
    /// * `secret` - 以 SEC 开头的密钥
    pub fn secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// 设置请求超时时间
    ///
    /// # 参数
    /// * `timeout` - 超时时间
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置重试策略
    ///
    /// # 参数
    /// * `max_attempts` - 最大重试次数
    /// * `base_delay` - 基础延迟时间（指数退避）
    pub fn retry(mut self, max_attempts: u32, base_delay: Duration) -> Self {
        self.retry = Some(RetryConfig {
            max_attempts,
            base_delay,
            max_delay: DEFAULT_MAX_RETRY_DELAY,
        });
        self
    }

    /// 禁用重试
    pub fn no_retry(mut self) -> Self {
        self.retry = None;
        self
    }
}

/// 钉钉客户端
///
/// 持有 HTTP 连接池，适合频繁发送场景。
pub struct Client {
    /// 配置
    config: Config,
    /// HTTP 客户端
    http: HttpClient,
    /// 渠道名称
    name: String,
}

impl Client {
    /// 创建钉钉客户端
    ///
    /// # 调用流程
    /// 1. 使用配置创建 HTTP 客户端
    /// 2. 设置超时时间
    /// 3. 返回客户端实例
    ///
    /// # 参数
    /// * `config` - 钉钉配置
    ///
    /// # 示例
    /// ```rust
    /// use notify_manager_rs::dingtalk::{Config, Client};
    ///
    /// let client = Client::new(Config::new("webhook_url").secret("SEC..."));
    /// ```
    pub fn new(config: Config) -> Self {
        let http = HttpClient::builder()
            .timeout(config.timeout)
            .build()
            .expect("failed to create http client");

        Self {
            config,
            http,
            name: "dingtalk".to_string(),
        }
    }

    /// 设置渠道名称
    ///
    /// 用于在 Sender 中区分多个钉钉渠道。
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送通用消息
    ///
    /// # 调用流程
    /// 1. 将 Message 转换为钉钉消息格式
    /// 2. 生成签名（如果配置了密钥）
    /// 3. 发送 HTTP POST 请求
    /// 4. 解析响应，判断是否成功
    /// 5. 失败时根据配置进行重试
    ///
    /// # 参数
    /// * `message` - 通用消息
    ///
    /// # 返回
    /// * `Ok(())` - 发送成功
    /// * `Err(ChannelError)` - 发送失败
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        // 转换为钉钉消息格式
        let ding_msg = if message.is_markdown() {
            DingTalkRequest::markdown(
                message.title.as_deref().unwrap_or("通知"),
                &message.content,
            )
        } else {
            DingTalkRequest::text(&message.content)
        };

        self.send_request(&ding_msg).await
    }

    /// 发送钉钉原生消息
    ///
    /// 支持钉钉特有功能，如 @ 指定人员。
    ///
    /// # 参数
    /// * `message` - 钉钉原生消息
    ///
    /// # 示例
    /// ```rust,ignore
    /// use notify_manager_rs::dingtalk::{Client, Config, TextMsg};
    ///
    /// let client = Client::new(Config::new("url"));
    /// client.send_native(&TextMsg {
    ///     content: "告警".into(),
    ///     at_mobiles: vec!["138xxxx".into()],
    ///     at_user_ids: vec![],
    ///     is_at_all: false,
    /// }).await?;
    /// ```
    pub async fn send_native<M: Serialize>(&self, message: &M) -> Result<(), ChannelError> {
        self.send_request(message).await
    }

    /// 发送请求（内部方法）
    ///
    /// # 调用流程
    /// 1. 构建请求 URL（添加签名参数）
    /// 2. 发送 POST 请求
    /// 3. 解析响应
    /// 4. 失败时重试（如果可重试且配置了重试）
    async fn send_request<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        let url = self.build_url()?;

        debug!(url = %url, "sending dingtalk message");

        // 执行请求（带重试）
        match &self.config.retry {
            Some(retry_config) => self.send_with_retry(&url, body, retry_config).await,
            None => self.do_send(&url, body).await,
        }
    }

    /// 带重试的发送
    async fn send_with_retry<T: Serialize>(
        &self,
        url: &str,
        body: &T,
        retry_config: &RetryConfig,
    ) -> Result<(), ChannelError> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < retry_config.max_attempts {
            attempts += 1;

            match self.do_send(url, body).await {
                Ok(()) => {
                    if attempts > 1 {
                        info!(attempts, "dingtalk message sent after retry");
                    }
                    return Ok(());
                }
                Err(e) => {
                    if !e.is_retryable() {
                        return Err(e);
                    }

                    last_error = Some(e);

                    if attempts < retry_config.max_attempts {
                        // 计算延迟（指数退避）
                        let delay = std::cmp::min(
                            retry_config.base_delay * 2u32.pow(attempts - 1),
                            retry_config.max_delay,
                        );
                        warn!(
                            attempt = attempts,
                            max_attempts = retry_config.max_attempts,
                            delay_ms = delay.as_millis(),
                            "dingtalk send failed, retrying"
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ChannelError::Other("unknown error".into())))
    }

    /// 执行单次发送
    async fn do_send<T: Serialize>(&self, url: &str, body: &T) -> Result<(), ChannelError> {
        let response = self
            .http
            .post(url)
            .json(body)
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

        debug!(status = %status, body = %body_text, "dingtalk response");

        // 解析响应
        let resp: DingTalkResponse =
            serde_json::from_str(&body_text).map_err(|e| {
                error!(error = %e, body = %body_text, "failed to parse dingtalk response");
                ChannelError::Other(format!("invalid response: {}", body_text))
            })?;

        if resp.errcode == 0 {
            info!("dingtalk message sent successfully");
            Ok(())
        } else {
            error!(
                errcode = resp.errcode,
                errmsg = %resp.errmsg,
                "dingtalk api error"
            );

            // 根据错误码判断类型
            match resp.errcode {
                310000 => Err(ChannelError::RateLimit { retry_after: None }),
                _ => Err(ChannelError::ServerError {
                    code: resp.errcode,
                    message: resp.errmsg,
                }),
            }
        }
    }

    /// 构建请求 URL（添加签名）
    fn build_url(&self) -> Result<String, ChannelError> {
        match &self.config.secret {
            Some(secret) => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| ChannelError::Other(format!("time error: {}", e)))?
                    .as_millis();

                let sign = self.calculate_sign(timestamp as i64, secret)?;

                Ok(format!(
                    "{}&timestamp={}&sign={}",
                    self.config.webhook_url, timestamp, sign
                ))
            }
            None => Ok(self.config.webhook_url.clone()),
        }
    }

    /// 计算签名
    fn calculate_sign(&self, timestamp: i64, secret: &str) -> Result<String, ChannelError> {
        use base64::Engine;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let string_to_sign = format!("{}\n{}", timestamp, secret);

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .map_err(|e| ChannelError::Other(format!("hmac error: {}", e)))?;
        mac.update(string_to_sign.as_bytes());

        let result = mac.finalize();
        let sign = base64::engine::general_purpose::STANDARD.encode(result.into_bytes());
        let sign_encoded = urlencoding::encode(&sign);

        Ok(sign_encoded.into_owned())
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
///
/// # 调用流程
/// 1. 创建临时 Client
/// 2. 发送消息
/// 3. 丢弃 Client
///
/// # 参数
/// * `config` - 钉钉配置
/// * `message` - 要发送的消息
///
/// # 示例
/// ```rust,ignore
/// use notify_manager_rs::dingtalk::{self, Config};
/// use notify_manager_rs::Message;
///
/// dingtalk::send(
///     &Config::new("webhook_url"),
///     &Message::text("告警")
/// ).await?;
/// ```
pub async fn send(config: &Config, message: &Message) -> Result<(), ChannelError> {
    let client = Client::new(config.clone());
    client.send(message).await
}

/// 钉钉 API 请求体
#[derive(Debug, Serialize)]
struct DingTalkRequest {
    msgtype: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<TextContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    markdown: Option<MarkdownContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    at: Option<AtConfig>,
}

#[derive(Debug, Serialize)]
struct TextContent {
    content: String,
}

#[derive(Debug, Serialize)]
struct MarkdownContent {
    title: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct AtConfig {
    #[serde(rename = "atMobiles", skip_serializing_if = "Vec::is_empty")]
    at_mobiles: Vec<String>,
    #[serde(rename = "atUserIds", skip_serializing_if = "Vec::is_empty")]
    at_user_ids: Vec<String>,
    #[serde(rename = "isAtAll")]
    is_at_all: bool,
}

impl DingTalkRequest {
    /// 创建文本消息
    fn text(content: &str) -> Self {
        Self {
            msgtype: "text".to_string(),
            text: Some(TextContent {
                content: content.to_string(),
            }),
            markdown: None,
            at: None,
        }
    }

    /// 创建 Markdown 消息
    fn markdown(title: &str, text: &str) -> Self {
        Self {
            msgtype: "markdown".to_string(),
            text: None,
            markdown: Some(MarkdownContent {
                title: title.to_string(),
                text: text.to_string(),
            }),
            at: None,
        }
    }
}

/// 钉钉 API 响应
#[derive(Debug, Deserialize)]
struct DingTalkResponse {
    errcode: i32,
    errmsg: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：配置创建
    #[test]
    fn test_config_builder() {
        let config = Config::new("https://example.com")
            .secret("SEC123")
            .timeout(Duration::from_secs(5))
            .retry(5, Duration::from_millis(100));

        assert_eq!(config.webhook_url, "https://example.com");
        assert_eq!(config.secret, Some("SEC123".to_string()));
        assert_eq!(config.timeout, Duration::from_secs(5));
        assert!(config.retry.is_some());
    }

    /// 测试：禁用重试
    #[test]
    fn test_no_retry() {
        let config = Config::new("url").no_retry();
        assert!(config.retry.is_none());
    }
}
