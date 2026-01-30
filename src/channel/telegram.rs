//! # Telegram 渠道模块
//!
//! 通过 Telegram Bot API 发送消息。
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::{telegram, Message};
//!
//! let config = telegram::Config::new("YOUR_BOT_TOKEN", "CHAT_ID");
//! let client = telegram::Client::new(config);
//! client.send(&Message::text("Hello Telegram!")).await?;
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::channel::Channel;
use crate::error::ChannelError;
use crate::message::Message;

/// Telegram 配置
#[derive(Debug, Clone)]
pub struct Config {
    /// Bot Token
    bot_token: String,
    /// Chat ID（可以是用户 ID、群组 ID 或频道 @username）
    chat_id: String,
    /// 解析模式（HTML 或 Markdown）
    parse_mode: Option<String>,
    /// 禁用链接预览
    disable_web_page_preview: bool,
    /// 禁用通知
    disable_notification: bool,
    /// 超时时间（秒）
    timeout_secs: u64,
    /// 重试次数
    max_retries: u32,
}

impl Config {
    /// 创建 Telegram 配置
    pub fn new(bot_token: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            chat_id: chat_id.into(),
            parse_mode: Some("HTML".to_string()),
            disable_web_page_preview: false,
            disable_notification: false,
            timeout_secs: 30,
            max_retries: 3,
        }
    }

    /// 设置解析模式（HTML 或 MarkdownV2）
    pub fn parse_mode(mut self, mode: impl Into<String>) -> Self {
        self.parse_mode = Some(mode.into());
        self
    }

    /// 禁用链接预览
    pub fn disable_preview(mut self) -> Self {
        self.disable_web_page_preview = true;
        self
    }

    /// 静默发送（不通知）
    pub fn silent(mut self) -> Self {
        self.disable_notification = true;
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

/// Telegram 消息请求
#[derive(Debug, Serialize)]
struct TelegramMessage {
    chat_id: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    disable_web_page_preview: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    disable_notification: bool,
}

/// Telegram API 响应
#[derive(Debug, Deserialize)]
struct TelegramResponse {
    ok: bool,
    #[serde(default)]
    description: Option<String>,
}

/// Telegram 客户端
pub struct Client {
    config: Config,
    http: reqwest::Client,
    name: String,
}

impl Client {
    /// 创建 Telegram 客户端
    pub fn new(config: Config) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("failed to create http client");

        Self {
            config,
            http,
            name: "telegram".to_string(),
        }
    }

    /// 设置渠道名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送通用消息
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        let text = if let Some(ref title) = message.title {
            format!("<b>{}</b>\n\n{}", escape_html(title), escape_html(&message.content))
        } else {
            escape_html(&message.content)
        };

        let telegram_msg = TelegramMessage {
            chat_id: self.config.chat_id.clone(),
            text,
            parse_mode: self.config.parse_mode.clone(),
            disable_web_page_preview: self.config.disable_web_page_preview,
            disable_notification: self.config.disable_notification,
        };

        self.send_with_retry(&telegram_msg).await
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
                        warn!(attempt = attempt, "telegram send failed, retrying");
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ChannelError::Other("unknown error".into())))
    }

    async fn do_send<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.config.bot_token
        );

        debug!(chat_id = %self.config.chat_id, "sending telegram message");

        let response = self
            .http
            .post(&url)
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

        debug!(status = %status, body = %body_text, "telegram response");

        let resp: TelegramResponse = serde_json::from_str(&body_text).map_err(|e| {
            error!(error = %e, "failed to parse telegram response");
            ChannelError::Other(format!("invalid response: {}", body_text))
        })?;

        if resp.ok {
            info!("telegram message sent successfully");
            Ok(())
        } else {
            let msg = resp.description.unwrap_or_else(|| "unknown error".into());
            error!(error = %msg, "telegram api error");
            Err(ChannelError::ServerError {
                code: status.as_u16() as i32,
                message: msg,
            })
        }
    }
}

/// HTML 转义
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
        let config = Config::new("123456:ABC-DEF", "987654321")
            .parse_mode("MarkdownV2")
            .disable_preview()
            .silent();

        assert_eq!(config.bot_token, "123456:ABC-DEF");
        assert_eq!(config.chat_id, "987654321");
        assert!(config.disable_web_page_preview);
        assert!(config.disable_notification);
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }
}
