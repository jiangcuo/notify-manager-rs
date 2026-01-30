//! # Discord 渠道模块
//!
//! 通过 Discord Webhook 发送消息。
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::{discord, Message};
//!
//! let config = discord::Config::new("https://discord.com/api/webhooks/xxx/yyy");
//! let client = discord::Client::new(config);
//! client.send(&Message::text("Hello Discord!")).await?;
//! ```

use async_trait::async_trait;
use serde::Serialize;
use tracing::{debug, error, info, warn};

use crate::channel::Channel;
use crate::error::ChannelError;
use crate::message::Message;

/// Discord 配置
#[derive(Debug, Clone)]
pub struct Config {
    /// Webhook URL
    webhook_url: String,
    /// 用户名（可选）
    username: Option<String>,
    /// 头像 URL（可选）
    avatar_url: Option<String>,
    /// 超时时间（秒）
    timeout_secs: u64,
    /// 重试次数
    max_retries: u32,
}

impl Config {
    /// 创建 Discord 配置
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            username: None,
            avatar_url: None,
            timeout_secs: 30,
            max_retries: 3,
        }
    }

    /// 设置用户名
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// 设置头像 URL
    pub fn avatar_url(mut self, url: impl Into<String>) -> Self {
        self.avatar_url = Some(url.into());
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

/// Discord 消息请求
#[derive(Debug, Serialize)]
struct DiscordMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embeds: Option<Vec<Embed>>,
}

#[derive(Debug, Serialize)]
struct Embed {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    description: String,
    color: u32,
}

/// Discord 客户端
pub struct Client {
    config: Config,
    http: reqwest::Client,
    name: String,
}

impl Client {
    /// 创建 Discord 客户端
    pub fn new(config: Config) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("failed to create http client");

        Self {
            config,
            http,
            name: "discord".to_string(),
        }
    }

    /// 设置渠道名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送通用消息
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        // Discord embed 颜色 (decimal)
        let color = match message.level {
            crate::message::Level::Info => 0x36a64f,    // green
            crate::message::Level::Warning => 0xffcc00, // yellow
            crate::message::Level::Error => 0xff0000,   // red
            crate::message::Level::Critical => 0x8b0000, // dark red
        };

        let discord_msg = if let Some(ref title) = message.title {
            DiscordMessage {
                content: None,
                username: self.config.username.clone(),
                avatar_url: self.config.avatar_url.clone(),
                embeds: Some(vec![Embed {
                    title: Some(title.clone()),
                    description: message.content.clone(),
                    color,
                }]),
            }
        } else {
            DiscordMessage {
                content: Some(message.content.clone()),
                username: self.config.username.clone(),
                avatar_url: self.config.avatar_url.clone(),
                embeds: None,
            }
        };

        self.send_with_retry(&discord_msg).await
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
                        warn!(attempt = attempt, "discord send failed, retrying");
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ChannelError::Other("unknown error".into())))
    }

    async fn do_send<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        debug!(url = %self.config.webhook_url, "sending discord message");

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

        // Discord returns 204 No Content on success
        if status.is_success() {
            info!("discord message sent successfully");
            Ok(())
        } else {
            let body_text = response.text().await.unwrap_or_default();
            error!(status = %status, body = %body_text, "discord api error");
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
        let config = Config::new("https://discord.com/api/webhooks/xxx")
            .username("AlertBot")
            .avatar_url("https://example.com/avatar.png");

        assert_eq!(config.webhook_url, "https://discord.com/api/webhooks/xxx");
        assert_eq!(config.username, Some("AlertBot".to_string()));
    }
}
