//! # 消息结构模块
//!
//! 定义通用消息格式，支持跨渠道发送。
//!
//! ## 消息类型
//! - `Message` - 通用消息结构
//! - `Level` - 消息级别（Info/Warning/Error/Critical）
//!
//! ## 使用示例
//! ```rust
//! use notify_manager_rs::{Message, Level};
//!
//! // 简单文本消息
//! let msg = Message::text("服务器告警");
//!
//! // 带标题的消息
//! let msg = Message::new("磁盘告警", "使用率达到 95%");
//!
//! // Builder 模式
//! let msg = Message::builder()
//!     .title("CPU 告警")
//!     .content("使用率超过 90%")
//!     .level(Level::Warning)
//!     .extra("host", "server-01")
//!     .build();
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 消息级别
///
/// 用于标识消息的严重程度，部分渠道会根据级别调整展示样式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// 普通信息
    #[default]
    Info,
    /// 警告
    Warning,
    /// 错误
    Error,
    /// 严重/紧急
    Critical,
}

impl Level {
    /// 获取级别的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warning => "warning",
            Level::Error => "error",
            Level::Critical => "critical",
        }
    }
}

/// 通用消息结构
///
/// 跨渠道的统一消息格式，包含标题、内容、级别和扩展字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息标题（部分渠道支持）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// 消息正文
    pub content: String,

    /// 消息级别
    #[serde(default)]
    pub level: Level,

    /// 扩展字段（Key-Value）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

impl Message {
    /// 创建带标题的消息
    ///
    /// # 参数
    /// * `title` - 消息标题
    /// * `content` - 消息内容
    ///
    /// # 示例
    /// ```rust
    /// use notify_manager_rs::Message;
    ///
    /// let msg = Message::new("告警", "磁盘使用率 95%");
    /// ```
    pub fn new(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: Some(title.into()),
            content: content.into(),
            level: Level::default(),
            extra: HashMap::new(),
        }
    }

    /// 创建纯文本消息（无标题）
    ///
    /// # 参数
    /// * `content` - 消息内容
    ///
    /// # 示例
    /// ```rust
    /// use notify_manager_rs::Message;
    ///
    /// let msg = Message::text("服务重启完成");
    /// ```
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            title: None,
            content: content.into(),
            level: Level::default(),
            extra: HashMap::new(),
        }
    }

    /// 创建 Markdown 格式消息
    ///
    /// # 参数
    /// * `title` - 标题
    /// * `markdown` - Markdown 格式内容
    ///
    /// # 示例
    /// ```rust
    /// use notify_manager_rs::Message;
    ///
    /// let msg = Message::markdown("告警", "**服务器** `server-01` CPU 超载");
    /// ```
    pub fn markdown(title: impl Into<String>, markdown: impl Into<String>) -> Self {
        let mut extra = HashMap::new();
        extra.insert("format".to_string(), "markdown".to_string());

        Self {
            title: Some(title.into()),
            content: markdown.into(),
            level: Level::default(),
            extra,
        }
    }

    /// 使用 Builder 模式构建消息
    ///
    /// # 示例
    /// ```rust
    /// use notify_manager_rs::{Message, Level};
    ///
    /// let msg = Message::builder()
    ///     .title("告警")
    ///     .content("磁盘满了")
    ///     .level(Level::Critical)
    ///     .build();
    /// ```
    pub fn builder() -> MessageBuilder {
        MessageBuilder::default()
    }

    /// 判断是否为 Markdown 格式
    pub fn is_markdown(&self) -> bool {
        self.extra.get("format").map(|f| f == "markdown").unwrap_or(false)
    }

    /// 设置消息级别（链式调用）
    pub fn with_level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }
}

/// 消息构建器
///
/// 使用 Builder 模式构建消息。
#[derive(Debug, Default)]
pub struct MessageBuilder {
    title: Option<String>,
    content: Option<String>,
    level: Level,
    extra: HashMap<String, String>,
}

impl MessageBuilder {
    /// 设置标题
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 设置内容
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// 设置级别
    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// 添加扩展字段
    pub fn extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    /// 设置为 Markdown 格式
    pub fn markdown(mut self) -> Self {
        self.extra.insert("format".to_string(), "markdown".to_string());
        self
    }

    /// 构建消息
    ///
    /// # Panics
    /// 如果 content 未设置会 panic
    pub fn build(self) -> Message {
        Message {
            title: self.title,
            content: self.content.expect("message content is required"),
            level: self.level,
            extra: self.extra,
        }
    }

    /// 尝试构建消息
    ///
    /// # 返回
    /// - `Some(Message)` - 构建成功
    /// - `None` - content 未设置
    pub fn try_build(self) -> Option<Message> {
        Some(Message {
            title: self.title,
            content: self.content?,
            level: self.level,
            extra: self.extra,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：创建文本消息
    #[test]
    fn test_text_message() {
        let msg = Message::text("hello");
        assert_eq!(msg.content, "hello");
        assert!(msg.title.is_none());
        assert_eq!(msg.level, Level::Info);
    }

    /// 测试：创建带标题消息
    #[test]
    fn test_new_message() {
        let msg = Message::new("Title", "Content");
        assert_eq!(msg.title, Some("Title".to_string()));
        assert_eq!(msg.content, "Content");
    }

    /// 测试：Builder 模式
    #[test]
    fn test_builder() {
        let msg = Message::builder()
            .title("Alert")
            .content("CPU high")
            .level(Level::Warning)
            .extra("host", "server-01")
            .build();

        assert_eq!(msg.title, Some("Alert".to_string()));
        assert_eq!(msg.content, "CPU high");
        assert_eq!(msg.level, Level::Warning);
        assert_eq!(msg.extra.get("host"), Some(&"server-01".to_string()));
    }

    /// 测试：Markdown 消息
    #[test]
    fn test_markdown_message() {
        let msg = Message::markdown("Title", "**bold**");
        assert!(msg.is_markdown());
    }
}
