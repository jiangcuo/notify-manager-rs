//! # Slack 渠道模块
//!
//! 通过 Slack Incoming Webhook 发送消息。
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::{slack, Message};
//!
//! let config = slack::Config::new("https://hooks.slack.com/services/xxx/yyy/zzz");
//! let client = slack::Client::new(config);
//! client.send(&Message::text("Hello Slack!")).await?;
//! ```

use async_trait::async_trait;
use serde::Serialize;
use tracing::{debug, error, info, warn};

use crate::channel::Channel;
use crate::error::ChannelError;
use crate::message::Message;

/// Slack 配置
#[derive(Debug, Clone)]
pub struct Config {
    /// Webhook URL
    webhook_url: String,
    /// 频道（可选，覆盖 webhook 默认频道）
    channel: Option<String>,
    /// 用户名（可选）
    username: Option<String>,
    /// 图标 emoji（可选）
    icon_emoji: Option<String>,
    /// 超时时间（秒）
    timeout_secs: u64,
    /// 重试次数
    max_retries: u32,
}

impl Config {
    /// 创建 Slack 配置
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            channel: None,
            username: None,
            icon_emoji: None,
            timeout_secs: 30,
            max_retries: 3,
        }
    }

    /// 设置频道
    pub fn channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    /// 设置用户名
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// 设置图标 emoji
    pub fn icon_emoji(mut self, emoji: impl Into<String>) -> Self {
        self.icon_emoji = Some(emoji.into());
        self
    }

    /// 设置超时时间
    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// 设置重试次数
    pub fn retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
}

/// Slack 消息请求
#[derive(Debug, Serialize)]
struct SlackMessage {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_emoji: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachments: Option<Vec<Attachment>>,
}

#[derive(Debug, Serialize)]
struct Attachment {
    color: String,
    title: Option<String>,
    text: String,
}

/// Slack 客户端
pub struct Client {
    config: Config,
    http: reqwest::Client,
    name: String,
}

impl Client {
    /// 创建 Slack 客户端
    pub fn new(config: Config) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("failed to create http client");

        Self {
            config,
            http,
            name: "slack".to_string(),
        }
    }

    /// 设置渠道名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送通用消息
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        let color = match message.level {
            crate::message::Level::Info => "#36a64f",
            crate::message::Level::Warning => "#ffcc00",
            crate::message::Level::Error => "#ff0000",
            crate::message::Level::Critical => "#8b0000",
        };

        let slack_msg = if let Some(ref title) = message.title {
            SlackMessage {
                text: String::new(),
                channel: self.config.channel.clone(),
                username: self.config.username.clone(),
                icon_emoji: self.config.icon_emoji.clone(),
                attachments: Some(vec![Attachment {
                    color: color.to_string(),
                    title: Some(title.clone()),
                    text: message.content.clone(),
                }]),
            }
        } else {
            SlackMessage {
                text: message.content.clone(),
                channel: self.config.channel.clone(),
                username: self.config.username.clone(),
                icon_emoji: self.config.icon_emoji.clone(),
                attachments: None,
            }
        };

        self.send_with_retry(&slack_msg).await
    }

    async fn send_with_retry<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        let mut last_error = None;

        for attempt in 1..=self.config.max_retries {
            match self.do_send(body).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.config.max_retries {
                        let delay = std::time::Duration::from_millis(100 * 2u64.pow(attempt - 1));
                        warn!(attempt = attempt, "slack send failed, retrying");
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ChannelError::Other("unknown error".into())))
    }

    async fn do_send<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        debug!(url = %self.config.webhook_url, "sending slack message");

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

        debug!(status = %status, body = %body_text, "slack response");

        if status.is_success() && body_text == "ok" {
            info!("slack message sent successfully");
            Ok(())
        } else {
            error!(status = %status, body = %body_text, "slack api error");
            Err(ChannelError::ServerError {
                code: status.as_u16() as i32,
                message: body_text,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = Config::new("https://hooks.slack.com/xxx")
            .channel("#general")
            .username("bot")
            .icon_emoji(":robot:");

        assert_eq!(config.webhook_url, "https://hooks.slack.com/xxx");
        assert_eq!(config.channel, Some("#general".to_string()));
        assert_eq!(config.username, Some("bot".to_string()));
    }
}
