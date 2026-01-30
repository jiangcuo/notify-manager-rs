//! # 企业微信渠道模块
//!
//! 提供企业微信机器人 Webhook 消息推送功能。
//!
//! ## 功能特性
//! - 支持 Text / Markdown / Image / News 消息类型
//! - 支持 @ 指定人员
//! - 自动重试机制
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::wecom;
//! use notify_manager_rs::Message;
//!
//! let config = wecom::Config::new("webhook_url");
//! let client = wecom::Client::new(config);
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

/// 企业微信渠道配置
#[derive(Debug, Clone)]
pub struct Config {
    webhook_url: String,
    timeout: Duration,
    retry: Option<RetryConfig>,
}

impl Config {
    /// 创建企业微信配置
    ///
    /// # 参数
    /// * `webhook_url` - 企业微信机器人 Webhook URL
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            timeout: DEFAULT_TIMEOUT,
            retry: Some(RetryConfig::default()),
        }
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

/// 企业微信客户端
pub struct Client {
    config: Config,
    http: HttpClient,
    name: String,
}

impl Client {
    /// 创建企业微信客户端
    pub fn new(config: Config) -> Self {
        let http = HttpClient::builder()
            .timeout(config.timeout)
            .build()
            .expect("failed to create http client");

        Self {
            config,
            http,
            name: "wecom".to_string(),
        }
    }

    /// 设置渠道名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送通用消息
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        let wecom_msg = if message.is_markdown() {
            WecomRequest::markdown(&message.content)
        } else {
            WecomRequest::text(&message.content)
        };

        self.send_request(&wecom_msg).await
    }

    /// 发送企业微信原生消息
    pub async fn send_native<M: Serialize>(&self, message: &M) -> Result<(), ChannelError> {
        self.send_request(message).await
    }

    async fn send_request<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        debug!(url = %self.config.webhook_url, "sending wecom message");

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
                        info!(attempts, "wecom message sent after retry");
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
                        warn!(attempt = attempts, "wecom send failed, retrying");
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ChannelError::Other("unknown error".into())))
    }

    async fn do_send<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        let response = self
            .http
            .post(&self.config.webhook_url)
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

        debug!(status = %status, body = %body_text, "wecom response");

        let resp: WecomResponse = serde_json::from_str(&body_text).map_err(|e| {
            error!(error = %e, "failed to parse wecom response");
            ChannelError::Other(format!("invalid response: {}", body_text))
        })?;

        if resp.errcode == 0 {
            info!("wecom message sent successfully");
            Ok(())
        } else {
            error!(errcode = resp.errcode, errmsg = %resp.errmsg, "wecom api error");
            Err(ChannelError::ServerError {
                code: resp.errcode,
                message: resp.errmsg,
            })
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
pub async fn send(config: &Config, message: &Message) -> Result<(), ChannelError> {
    let client = Client::new(config.clone());
    client.send(message).await
}

#[derive(Debug, Serialize)]
struct WecomRequest {
    msgtype: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<TextContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    markdown: Option<MarkdownContent>,
}

#[derive(Debug, Serialize)]
struct TextContent {
    content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mentioned_list: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mentioned_mobile_list: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MarkdownContent {
    content: String,
}

impl WecomRequest {
    fn text(content: &str) -> Self {
        Self {
            msgtype: "text".to_string(),
            text: Some(TextContent {
                content: content.to_string(),
                mentioned_list: Vec::new(),
                mentioned_mobile_list: Vec::new(),
            }),
            markdown: None,
        }
    }

    fn markdown(content: &str) -> Self {
        Self {
            msgtype: "markdown".to_string(),
            text: None,
            markdown: Some(MarkdownContent {
                content: content.to_string(),
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
struct WecomResponse {
    errcode: i32,
    errmsg: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = Config::new("https://example.com").timeout(Duration::from_secs(5));
        assert_eq!(config.webhook_url, "https://example.com");
        assert_eq!(config.timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_text_request() {
        let req = WecomRequest::text("hello");
        assert_eq!(req.msgtype, "text");
    }

    #[test]
    fn test_markdown_request() {
        let req = WecomRequest::markdown("**bold**");
        assert_eq!(req.msgtype, "markdown");
    }
}
