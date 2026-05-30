//! # 短信渠道模块
//!
//! 提供短信发送能力，按服务商分子模块组织，便于未来扩展多个短信平台。
//!
//! ## 支持的服务商
//! - [`aliyun`] - 阿里云短信（dysmsapi，HMAC-SHA1 RPC 签名）
//!
//! ## 设计说明
//!
//! 短信和其他渠道（钉钉/邮件等）的模型不同：短信**不能发送自由文本**，
//! 必须使用「签名(SignName) + 模板(TemplateCode) + 模板参数(TemplateParam)」。
//!
//! 因此通用 [`Message`](crate::Message) 到短信的映射约定如下：
//!
//! | Message 字段 | 映射到短信 |
//! |--------------|-----------|
//! | `extra` 中的键值对 | 模板参数 `TemplateParam`（JSON），保留键除外 |
//! | `extra["sign_name"]` | 覆盖配置中的签名（可选） |
//! | `extra["template_code"]` | 覆盖配置中的模板 ID（可选） |
//! | `content` / `title` | 仅用于日志，不参与发送 |
//!
//! 保留键（不会作为模板参数）：`format`、`sign_name`、`template_code`。
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::sms::aliyun;
//! use notify_manager_rs::Message;
//!
//! let config = aliyun::Config::new("access_key_id", "access_key_secret")
//!     .region("cn-hangzhou")
//!     .sign_name("梨儿方")
//!     .template_code("SMS_123456789")
//!     .to("13800138000");
//!
//! // extra 中的键值对会拼成模板参数：{"code":"123456"}
//! let msg = Message::builder()
//!     .content("验证码通知")
//!     .extra("code", "123456")
//!     .build();
//!
//! aliyun::Client::new(config).send(&msg).await?;
//! ```

/// 短信渠道公共保留键。
///
/// 这些键出现在 [`Message::extra`](crate::Message) 中时不会作为模板参数发送，
/// 而是用于控制发送行为。
pub(crate) const RESERVED_KEYS: [&str; 3] = ["format", "sign_name", "template_code"];

// 各服务商实现
pub mod aliyun;
