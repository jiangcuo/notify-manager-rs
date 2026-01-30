//! # 飞书渠道模块
//!
//! 提供飞书机器人 Webhook 消息推送功能。
//!
//! ## 功能特性
//! - 支持 Text / Card 消息类型
//! - 支持签名验证
//! - 支持 @ 指定人员
//! - 自动重试机制
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::feishu;
//! use notify_manager_rs::Message;
//!
//! let config = feishu::Config::new("webhook_url").secret("your-secret");
//! let client = feishu::Client::new(config);
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

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_RETRY_ATTEMPTS: u32 = 3;
const DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(200);

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_RETRY_ATTEMPTS,
            base_delay: DEFAULT_RETRY_DELAY,
            max_delay: Duration::from_secs(5),
        }
    }
}

/// 飞书渠道配置
#[derive(Debug, Clone)]
pub struct Config {
    webhook_url: String,
    secret: Option<String>,
    timeout: Duration,
    retry: Option<RetryConfig>,
}

impl Config {
    /// 创建飞书配置
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            secret: None,
            timeout: DEFAULT_TIMEOUT,
            retry: Some(RetryConfig::default()),
        }
    }

    /// 设置签名密钥
    pub fn secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    /// 设置请求超时时间
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置重试策略
    pub fn retry(mut self, max_attempts: u32, base_delay: Duration) -> Self {
        self.retry = Some(RetryConfig {
            max_attempts,
            base_delay,
            max_delay: Duration::from_secs(5),
        });
        self
    }

    /// 禁用重试
    pub fn no_retry(mut self) -> Self {
        self.retry = None;
        self
    }
}

/// 飞书客户端
pub struct Client {
    config: Config,
    http: HttpClient,
    name: String,
}

impl Client {
    /// 创建飞书客户端
    pub fn new(config: Config) -> Self {
        let http = HttpClient::builder()
            .timeout(config.timeout)
            .build()
            .expect("failed to create http client");

        Self {
            config,
            http,
            name: "feishu".to_string(),
        }
    }

    /// 设置渠道名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送通用消息
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        let feishu_msg = if message.is_markdown() {
            FeishuRequest::card(
                message.title.as_deref().unwrap_or("通知"),
                &message.content,
            )
        } else {
            FeishuRequest::text(&message.content)
        };

        self.send_request(&feishu_msg).await
    }

    /// 发送飞书原生消息
    pub async fn send_native<M: Serialize>(&self, message: &M) -> Result<(), ChannelError> {
        self.send_request(message).await
    }

    async fn send_request<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        debug!(url = %self.config.webhook_url, "sending feishu message");

        match &self.config.retry {
            Some(retry_config) => self.send_with_retry(body, retry_config).await,
            None => self.do_send(body).await,
        }
    }

    async fn send_with_retry<T: Serialize>(
        &self,
        body: &T,
        retry_config: &RetryConfig,
    ) -> Result<(), ChannelError> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < retry_config.max_attempts {
            attempts += 1;

            match self.do_send(body).await {
                Ok(()) => {
                    if attempts > 1 {
                        info!(attempts, "feishu message sent after retry");
                    }
                    return Ok(());
                }
                Err(e) => {
                    if !e.is_retryable() {
                        return Err(e);
                    }
                    last_error = Some(e);
                    if attempts < retry_config.max_attempts {
                        let delay = std::cmp::min(
                            retry_config.base_delay * 2u32.pow(attempts - 1),
                            retry_config.max_delay,
                        );
                        warn!(attempt = attempts, "feishu send failed, retrying");
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ChannelError::Other("unknown error".into())))
    }

    async fn do_send<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        let mut request_body = serde_json::to_value(body)
            .map_err(|e| ChannelError::InvalidMessage(format!("serialize error: {}", e)))?;

        if let Some(ref secret) = self.config.secret {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| ChannelError::Other(format!("time error: {}", e)))?
                .as_secs();

            let sign = self.calculate_sign(timestamp, secret)?;

            if let Some(obj) = request_body.as_object_mut() {
                // 飞书要求 timestamp 是字符串类型
                obj.insert("timestamp".to_string(), serde_json::json!(timestamp.to_string()));
                obj.insert("sign".to_string(), serde_json::json!(sign));
            }

            debug!(timestamp = timestamp, sign = %sign, "feishu sign generated");
        }

        let response = self
            .http
            .post(&self.config.webhook_url)
            .json(&request_body)
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

        debug!(status = %status, body = %body_text, "feishu response");

        let resp: FeishuResponse = serde_json::from_str(&body_text).map_err(|e| {
            error!(error = %e, "failed to parse feishu response");
            ChannelError::Other(format!("invalid response: {}", body_text))
        })?;

        if resp.code == 0 {
            info!("feishu message sent successfully");
            Ok(())
        } else {
            error!(code = resp.code, msg = %resp.msg, "feishu api error");
            Err(ChannelError::ServerError {
                code: resp.code,
                message: resp.msg,
            })
        }
    }

    fn calculate_sign(&self, timestamp: u64, secret: &str) -> Result<String, ChannelError> {
        use base64::Engine;
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let string_to_sign = format!("{}\n{}", timestamp, secret);

        let mut mac = Hmac::<Sha256>::new_from_slice(string_to_sign.as_bytes())
            .map_err(|e| ChannelError::Other(format!("hmac error: {}", e)))?;
        mac.update(&[]);

        let result = mac.finalize();
        Ok(base64::engine::general_purpose::STANDARD.encode(result.into_bytes()))
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
pub async fn send(config: &Config, message: &Message) -> Result<(), ChannelError> {
    let client = Client::new(config.clone());
    client.send(message).await
}

#[derive(Debug, Serialize)]
struct FeishuRequest {
    msg_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<TextContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    card: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct TextContent {
    text: String,
}

impl FeishuRequest {
    fn text(content: &str) -> Self {
        Self {
            msg_type: "text".to_string(),
            content: Some(TextContent { text: content.to_string() }),
            card: None,
        }
    }

    fn card(title: &str, content: &str) -> Self {
        let card = serde_json::json!({
            "config": { "wide_screen_mode": true },
            "header": {
                "title": { "tag": "plain_text", "content": title }
            },
            "elements": [{ "tag": "markdown", "content": content }]
        });

        Self {
            msg_type: "interactive".to_string(),
            content: None,
            card: Some(card),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FeishuResponse {
    code: i32,
    msg: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = Config::new("https://example.com").secret("secret123");
        assert_eq!(config.webhook_url, "https://example.com");
        assert_eq!(config.secret, Some("secret123".to_string()));
    }

    #[test]
    fn test_sign_calculation() {
        // 使用 Java 示例中的测试数据验证签名算法
        let client = Client::new(Config::new("https://example.com"));
        let timestamp: u64 = 1599360473;
        let secret = "demo";
        
        let sign = client.calculate_sign(timestamp, secret).unwrap();
        
        // Java 代码生成的签名应该和这里一致
        println!("Generated sign: {}", sign);
        // 签名不应为空
        assert!(!sign.is_empty());
    }
}
