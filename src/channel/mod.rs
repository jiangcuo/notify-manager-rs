//! # 渠道模块
//!
//! 定义渠道 trait 和各渠道实现。
//!
//! ## 国内渠道
//! - `dingtalk` - 钉钉机器人
//! - `feishu` - 飞书机器人
//! - `wecom` - 企业微信机器人
//!
//! ## 国际渠道
//! - `slack` - Slack Webhook
//! - `discord` - Discord Webhook
//! - `telegram` - Telegram Bot
//!
//! ## 通用渠道
//! - `webhook` - 通用 Webhook
//! - `email` - SMTP 邮件
//! - `sms::aliyun` - 阿里云短信

use async_trait::async_trait;

use crate::error::ChannelError;
use crate::message::Message;

/// 渠道 trait
///
/// 所有渠道必须实现此 trait，用于统一管理和发送消息。
///
/// # 实现要求
/// - 必须是 `Send + Sync`，支持跨线程使用
/// - `send` 方法应处理重试逻辑（如果配置了重试）
///
/// # 示例
/// ```rust,ignore
/// use async_trait::async_trait;
/// use notify_manager_rs::{Channel, Message, ChannelError};
///
/// struct MyChannel;
///
/// #[async_trait]
/// impl Channel for MyChannel {
///     fn name(&self) -> &str {
///         "my_channel"
///     }
///
///     async fn send(&self, message: &Message) -> Result<(), ChannelError> {
///         // 实现发送逻辑
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait Channel: Send + Sync {
    /// 获取渠道名称
    ///
    /// 用于日志记录和 Sender 中的渠道路由。
    fn name(&self) -> &str;

    /// 发送消息
    ///
    /// # 调用流程
    /// 1. 将 Message 转换为渠道特定格式
    /// 2. 发送 HTTP 请求（或其他协议）
    /// 3. 解析响应，判断是否成功
    /// 4. 失败时根据配置进行重试
    ///
    /// # 参数
    /// * `message` - 要发送的消息
    ///
    /// # 返回
    /// * `Ok(())` - 发送成功
    /// * `Err(ChannelError)` - 发送失败
    async fn send(&self, message: &Message) -> Result<(), ChannelError>;
}

// 国内渠道
pub mod dingtalk;
pub mod feishu;
pub mod wecom;

// 国际渠道
pub mod discord;
pub mod slack;
pub mod telegram;

// 通用渠道
pub mod email;
pub mod sms;
pub mod webhook;
