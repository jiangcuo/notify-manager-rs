//! # 钉钉原生消息类型
//!
//! 支持钉钉特有功能，如 @ 指定人员。

use serde::Serialize;

/// 钉钉文本消息
///
/// 支持 @ 指定人员功能。
///
/// # 示例
/// ```rust
/// use notify_manager_rs::dingtalk::TextMsg;
///
/// let msg = TextMsg::new("服务器告警")
///     .at_mobiles(vec!["138xxxx"])
///     .at_all(false);
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct TextMsg {
    /// 消息类型
    msgtype: String,
    /// 文本内容
    text: TextMsgContent,
    /// @ 配置
    at: AtConfig,
}

#[derive(Debug, Clone, Serialize)]
struct TextMsgContent {
    content: String,
}

#[derive(Debug, Clone, Default, Serialize)]
struct AtConfig {
    #[serde(rename = "atMobiles", skip_serializing_if = "Vec::is_empty")]
    at_mobiles: Vec<String>,
    #[serde(rename = "atUserIds", skip_serializing_if = "Vec::is_empty")]
    at_user_ids: Vec<String>,
    #[serde(rename = "isAtAll")]
    is_at_all: bool,
}

impl TextMsg {
    /// 创建文本消息
    ///
    /// # 参数
    /// * `content` - 消息内容
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            msgtype: "text".to_string(),
            text: TextMsgContent {
                content: content.into(),
            },
            at: AtConfig::default(),
        }
    }

    /// 设置 @ 的手机号列表
    ///
    /// # 参数
    /// * `mobiles` - 手机号列表
    pub fn at_mobiles(mut self, mobiles: Vec<impl Into<String>>) -> Self {
        self.at.at_mobiles = mobiles.into_iter().map(|m| m.into()).collect();
        self
    }

    /// 设置 @ 的用户 ID 列表
    ///
    /// # 参数
    /// * `user_ids` - 用户 ID 列表
    pub fn at_user_ids(mut self, user_ids: Vec<impl Into<String>>) -> Self {
        self.at.at_user_ids = user_ids.into_iter().map(|u| u.into()).collect();
        self
    }

    /// 设置是否 @ 所有人
    ///
    /// # 参数
    /// * `is_at_all` - 是否 @ 所有人
    pub fn at_all(mut self, is_at_all: bool) -> Self {
        self.at.is_at_all = is_at_all;
        self
    }
}

/// 钉钉 Markdown 消息
///
/// 支持 Markdown 格式和 @ 功能。
///
/// # 示例
/// ```rust
/// use notify_manager_rs::dingtalk::MarkdownMsg;
///
/// let msg = MarkdownMsg::new("告警", "## 服务器异常\n- CPU: 90%")
///     .at_mobiles(vec!["138xxxx"]);
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct MarkdownMsg {
    /// 消息类型
    msgtype: String,
    /// Markdown 内容
    markdown: MarkdownMsgContent,
    /// @ 配置
    at: AtConfig,
}

#[derive(Debug, Clone, Serialize)]
struct MarkdownMsgContent {
    title: String,
    text: String,
}

impl MarkdownMsg {
    /// 创建 Markdown 消息
    ///
    /// # 参数
    /// * `title` - 标题（会在消息列表中显示）
    /// * `text` - Markdown 内容
    pub fn new(title: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            msgtype: "markdown".to_string(),
            markdown: MarkdownMsgContent {
                title: title.into(),
                text: text.into(),
            },
            at: AtConfig::default(),
        }
    }

    /// 设置 @ 的手机号列表
    pub fn at_mobiles(mut self, mobiles: Vec<impl Into<String>>) -> Self {
        self.at.at_mobiles = mobiles.into_iter().map(|m| m.into()).collect();
        self
    }

    /// 设置 @ 的用户 ID 列表
    pub fn at_user_ids(mut self, user_ids: Vec<impl Into<String>>) -> Self {
        self.at.at_user_ids = user_ids.into_iter().map(|u| u.into()).collect();
        self
    }

    /// 设置是否 @ 所有人
    pub fn at_all(mut self, is_at_all: bool) -> Self {
        self.at.is_at_all = is_at_all;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：文本消息序列化
    #[test]
    fn test_text_msg_serialize() {
        let msg = TextMsg::new("hello")
            .at_mobiles(vec!["138"])
            .at_all(false);

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"msgtype\":\"text\""));
        assert!(json.contains("\"content\":\"hello\""));
        assert!(json.contains("\"atMobiles\":[\"138\"]"));
    }

    /// 测试：Markdown 消息序列化
    #[test]
    fn test_markdown_msg_serialize() {
        let msg = MarkdownMsg::new("Title", "**bold**");

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"msgtype\":\"markdown\""));
        assert!(json.contains("\"title\":\"Title\""));
    }
}
