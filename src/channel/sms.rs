//! # 短信渠道模块（占位实现）
//!
//! 提供短信发送功能的接口定义，当前为占位实现。
//! 未来可对接阿里云短信、腾讯云短信等服务商。
//!
//! ## 调用流程
//! ```text
//! Config::new() → Client::new() → client.send() → HTTP API → 短信服务商
//! ```
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::sms;
//! use notify_manager_rs::Message;
//!
//! let config = sms::Config::new("https://sms-api.example.com")
//!     .api_key("your_key")
//!     .api_secret("your_secret")
//!     .sign_name("梨儿方")
//!     .to("13800138000");
//!
//! let client = sms::Client::new(config);
//! client.send(&Message::new("验证码", "您的验证码是 123456")).await?;
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::channel::Channel;
use crate::error::ChannelError;
use crate::message::Message;

/// 短信渠道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// API 地址
    api_url: String,
    /// API Key
    api_key: Option<String>,
    /// API Secret
    api_secret: Option<String>,
    /// 短信签名
    sign_name: Option<String>,
    /// 验证码模板 ID
    template_code: Option<String>,
    /// 收件人列表
    to: Vec<String>,
}

impl Config {
    /// 创建短信配置
    ///
    /// # 参数
    /// * `api_url` - 短信服务商 API 地址
    pub fn new(api_url: impl Into<String>) -> Self {
        Self {
            api_url: api_url.into(),
            api_key: None,
            api_secret: None,
            sign_name: None,
            template_code: None,
            to: Vec::new(),
        }
    }

    /// 设置 API Key
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// 设置 API Secret
    pub fn api_secret(mut self, secret: impl Into<String>) -> Self {
        self.api_secret = Some(secret.into());
        self
    }

    /// 设置短信签名
    pub fn sign_name(mut self, name: impl Into<String>) -> Self {
        self.sign_name = Some(name.into());
        self
    }

    /// 设置模板代码
    pub fn template_code(mut self, code: impl Into<String>) -> Self {
        self.template_code = Some(code.into());
        self
    }

    /// 添加收件人手机号
    pub fn to(mut self, phone: impl Into<String>) -> Self {
        self.to.push(phone.into());
        self
    }

    /// 添加多个收件人
    pub fn to_many(mut self, phones: Vec<impl Into<String>>) -> Self {
        self.to.extend(phones.into_iter().map(|p| p.into()));
        self
    }
}

/// 短信客户端（占位实现）
pub struct Client {
    config: Config,
    name: String,
}

impl Client {
    /// 创建短信客户端
    pub fn new(config: Config) -> Self {
        Self {
            config,
            name: "sms".to_string(),
        }
    }

    /// 设置渠道名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送短信（占位实现）
    ///
    /// 当前仅记录日志，不实际发送。
    /// 未来对接短信服务商后替换此实现。
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        if self.config.to.is_empty() {
            return Err(ChannelError::InvalidMessage(
                "at least one phone number is required".into(),
            ));
        }

        warn!(
            to = ?self.config.to,
            api_url = %self.config.api_url,
            "SMS channel is a placeholder - message not actually sent"
        );

        info!(
            to = ?self.config.to,
            content = %message.content,
            "SMS placeholder: would send message"
        );

        // 占位：返回未实现错误
        Err(ChannelError::Other(
            "SMS channel is not yet implemented. Please configure an email provider instead."
                .into(),
        ))
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

/// 一次性发送函数（占位）
pub async fn send(config: &Config, message: &Message) -> Result<(), ChannelError> {
    let client = Client::new(config.clone());
    client.send(message).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = Config::new("https://sms-api.example.com")
            .api_key("key123")
            .api_secret("secret456")
            .sign_name("梨儿方")
            .template_code("SMS_001")
            .to("13800138000");

        assert_eq!(config.api_url, "https://sms-api.example.com");
        assert_eq!(config.api_key, Some("key123".to_string()));
        assert_eq!(config.sign_name, Some("梨儿方".to_string()));
        assert_eq!(config.to.len(), 1);
    }
}
