//! # 邮件渠道模块
//!
//! 提供 SMTP 邮件发送功能。
//!
//! ## 功能特性
//! - 支持 SMTP/STARTTLS/SSL
//! - 支持多收件人
//! - 支持 HTML 和纯文本
//! - 支持抄送和密送
//!
//! ## 调用流程
//! ```text
//! Config::new() -> Client::new() -> client.send() -> SMTP 连接 -> 发送邮件
//! ```
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::email;
//! use notify_manager_rs::Message;
//!
//! let config = email::Config::new("smtp.example.com")
//!     .credentials("user@example.com", "password")
//!     .from("alert@example.com")
//!     .to("admin@example.com");
//!
//! let client = email::Client::new(config)?;
//! client.send(&Message::new("告警", "服务器异常")).await?;
//! ```

use async_trait::async_trait;
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message as LettreMessage, Tokio1Executor,
};
use tracing::{debug, error, info};

use crate::channel::Channel;
use crate::error::ChannelError;
use crate::message::Message;

/// 邮件渠道配置
#[derive(Debug, Clone)]
pub struct Config {
    /// SMTP 服务器地址
    smtp_host: String,
    /// SMTP 端口（默认 587）
    smtp_port: u16,
    /// 使用直接 TLS（true=TLS, false=STARTTLS）
    use_tls: bool,
    /// SMTP 认证凭证
    credentials: Option<Credentials>,
    /// 发件人地址
    from: Option<String>,
    /// 收件人列表
    to: Vec<String>,
    /// 抄送列表
    cc: Vec<String>,
    /// 密送列表
    bcc: Vec<String>,
}

impl Config {
    /// 创建邮件配置
    ///
    /// # 参数
    /// * `smtp_host` - SMTP 服务器地址
    pub fn new(smtp_host: impl Into<String>) -> Self {
        Self {
            smtp_host: smtp_host.into(),
            smtp_port: 587,
            use_tls: true,
            credentials: None,
            from: None,
            to: Vec::new(),
            cc: Vec::new(),
            bcc: Vec::new(),
        }
    }

    /// 设置 SMTP 端口
    pub fn port(mut self, port: u16) -> Self {
        self.smtp_port = port;
        self
    }

    /// 设置 TLS 模式
    ///
    /// - `true` - 直接 TLS/SSL 连接（适用于端口 465）
    /// - `false` - STARTTLS（适用于端口 587，默认）
    pub fn tls(mut self, use_tls: bool) -> Self {
        self.use_tls = use_tls;
        self
    }

    /// 设置 SMTP 认证凭证
    pub fn credentials(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.credentials = Some(Credentials::new(username.into(), password.into()));
        self
    }

    /// 设置发件人地址
    pub fn from(mut self, from: impl Into<String>) -> Self {
        self.from = Some(from.into());
        self
    }

    /// 添加收件人
    pub fn to(mut self, to: impl Into<String>) -> Self {
        self.to.push(to.into());
        self
    }

    /// 添加多个收件人
    pub fn to_many(mut self, recipients: Vec<impl Into<String>>) -> Self {
        self.to.extend(recipients.into_iter().map(|r| r.into()));
        self
    }

    /// 添加抄送
    pub fn cc(mut self, cc: impl Into<String>) -> Self {
        self.cc.push(cc.into());
        self
    }

    /// 添加密送
    pub fn bcc(mut self, bcc: impl Into<String>) -> Self {
        self.bcc.push(bcc.into());
        self
    }
}

/// 邮件客户端
pub struct Client {
    config: Config,
    transport: AsyncSmtpTransport<Tokio1Executor>,
    name: String,
}

impl Client {
    /// 创建邮件客户端
    ///
    /// # 调用流程
    /// 1. 根据配置创建 SMTP 传输器
    /// 2. 设置认证凭证（如果有）
    /// 3. 返回客户端实例
    pub fn new(config: Config) -> Result<Self, ChannelError> {
        if config.from.is_none() {
            return Err(ChannelError::InvalidMessage(
                "email 'from' address is required".into(),
            ));
        }
        if config.to.is_empty() {
            return Err(ChannelError::InvalidMessage(
                "at least one 'to' address is required".into(),
            ));
        }

        // tls(true) = 直接 TLS/SSL，tls(false) = STARTTLS
        let mut builder = if config.use_tls {
            // 直接 TLS/SSL 连接（适用于端口 465）
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
                .map_err(|e| ChannelError::Other(format!("smtp relay error: {}", e)))?
        } else {
            // STARTTLS（适用于端口 587）
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
                .map_err(|e| ChannelError::Other(format!("smtp relay error: {}", e)))?
        };

        builder = builder.port(config.smtp_port);

        if let Some(ref creds) = config.credentials {
            builder = builder.credentials(creds.clone());
        }

        let transport = builder.build();

        Ok(Self {
            config,
            transport,
            name: "email".to_string(),
        })
    }

    /// 设置渠道名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送通用消息
    ///
    /// # 调用流程
    /// 1. 将 Message 转换为邮件格式
    /// 2. 设置收件人、抄送、密送
    /// 3. 发送邮件
    /// 4. 解析发送结果
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        let from: Mailbox = self
            .config
            .from
            .as_ref()
            .unwrap()
            .parse()
            .map_err(|e| ChannelError::InvalidMessage(format!("invalid from address: {}", e)))?;

        let subject = message.title.as_deref().unwrap_or("Notification");
        let body = &message.content;

        let content_type = if message.is_markdown() {
            ContentType::TEXT_HTML
        } else {
            ContentType::TEXT_PLAIN
        };

        debug!(
            from = %from,
            to = ?self.config.to,
            subject = %subject,
            "preparing email"
        );

        let mut email_builder = LettreMessage::builder()
            .from(from)
            .subject(subject);

        for to_addr in &self.config.to {
            let mailbox: Mailbox = to_addr
                .parse()
                .map_err(|e| ChannelError::InvalidMessage(format!("invalid to address: {}", e)))?;
            email_builder = email_builder.to(mailbox);
        }

        for cc_addr in &self.config.cc {
            let mailbox: Mailbox = cc_addr
                .parse()
                .map_err(|e| ChannelError::InvalidMessage(format!("invalid cc address: {}", e)))?;
            email_builder = email_builder.cc(mailbox);
        }

        for bcc_addr in &self.config.bcc {
            let mailbox: Mailbox = bcc_addr
                .parse()
                .map_err(|e| ChannelError::InvalidMessage(format!("invalid bcc address: {}", e)))?;
            email_builder = email_builder.bcc(mailbox);
        }

        let email = email_builder
            .header(content_type)
            .body(body.clone())
            .map_err(|e| ChannelError::InvalidMessage(format!("failed to build email: {}", e)))?;

        self.transport
            .send(email)
            .await
            .map_err(|e| {
                error!(error = %e, "failed to send email");
                ChannelError::Email(format!("smtp error: {}", e))
            })?;

        info!(to = ?self.config.to, subject = %subject, "email sent successfully");
        Ok(())
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
/// 适合脚本或一次性任务。
pub async fn send(config: &Config, message: &Message) -> Result<(), ChannelError> {
    let client = Client::new(config.clone())?;
    client.send(message).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = Config::new("smtp.example.com")
            .port(465)
            .tls(true)
            .credentials("user", "pass")
            .from("from@example.com")
            .to("to@example.com")
            .cc("cc@example.com");

        assert_eq!(config.smtp_host, "smtp.example.com");
        assert_eq!(config.smtp_port, 465);
        assert!(config.use_tls);
        assert!(config.credentials.is_some());
        assert_eq!(config.from, Some("from@example.com".to_string()));
        assert_eq!(config.to.len(), 1);
        assert_eq!(config.cc.len(), 1);
    }

    #[test]
    fn test_missing_from() {
        let config = Config::new("smtp.example.com").to("to@example.com");
        let result = Client::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_to() {
        let config = Config::new("smtp.example.com").from("from@example.com");
        let result = Client::new(config);
        assert!(result.is_err());
    }
}
