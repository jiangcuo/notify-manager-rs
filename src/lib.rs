//! # notify-manager-rs
//!
//! 一个纯 Rust 实现的多渠道通知库，支持钉钉、Webhook、邮件等消息推送系统。
//!
//! ## 功能特性
//!
//! - **统一接口**：一套 API 发送到多个渠道
//! - **易于扩展**：新增渠道只需实现 trait
//! - **异步优先**：基于 `tokio` 的 async/await
//! - **生产可用**：完善的错误处理、重试、超时机制
//!
//! ## 分层 API
//!
//! | 层级 | API | 场景 |
//! |------|-----|------|
//! | Layer 1 | `dingtalk::send()` | 脚本、一次性任务 |
//! | Layer 2 | `dingtalk::Client` | 单渠道频繁发送 |
//! | Layer 3 | `Sender` | 多渠道管理 |
//!
//! ## 快速开始
//!
//! ### Layer 1：一次性发送
//!
//! ```rust,ignore
//! use notify_manager_rs::{dingtalk, Message};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), notify_manager_rs::Error> {
//!     let config = dingtalk::Config::new("https://oapi.dingtalk.com/robot/send?access_token=xxx");
//!     dingtalk::send(&config, &Message::text("告警")).await?;
//!     Ok(())
//! }
//! ```
//!
//! ### Layer 2：复用连接
//!
//! ```rust,ignore
//! use notify_manager_rs::{dingtalk, Message};
//!
//! let client = dingtalk::Client::new(
//!     dingtalk::Config::new("webhook_url").secret("SEC...")
//! );
//!
//! client.send(&Message::text("告警 1")).await?;
//! client.send(&Message::text("告警 2")).await?;
//! ```
//!
//! ### Layer 3：多渠道管理
//!
//! ```rust,ignore
//! use notify_manager_rs::{Sender, Message, dingtalk};
//!
//! let sender = Sender::new()
//!     .add("运维群", dingtalk::Client::new(config1))
//!     .add("开发群", dingtalk::Client::new(config2));
//!
//! // 广播到所有渠道
//! sender.send_all(&msg).await?;
//!
//! // 发送到指定渠道
//! sender.send_to("运维群", &msg).await?;
//! ```
//!
//! ## 支持的渠道
//!
//! | 渠道 | 描述 |
//! |------|------|
//! | `dingtalk` | 钉钉机器人 |
//! | `feishu` | 飞书机器人 |
//! | `wecom` | 企业微信机器人 |
//! | `slack` | Slack Webhook |
//! | `discord` | Discord Webhook |
//! | `telegram` | Telegram Bot |
//! | `webhook` | 通用 Webhook |
//! | `email` | SMTP 邮件 |

// 模块声明
mod channel;
mod error;
mod message;
mod sender;

// 公开导出
pub use channel::Channel;
pub use error::{ChannelError, Error};
pub use message::{Level, Message, MessageBuilder};
pub use sender::Sender;

// 国内渠道
pub use channel::dingtalk;
pub use channel::feishu;
pub use channel::wecom;

// 国际渠道
pub use channel::slack;
pub use channel::discord;
pub use channel::telegram;

// 通用渠道
pub use channel::email;
pub use channel::sms;
pub use channel::webhook;
