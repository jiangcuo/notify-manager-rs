//! # 飞书原生消息类型
//!
//! 支持飞书特有功能，如富文本、卡片消息、@ 人等。

use serde::Serialize;

/// 飞书文本消息（支持 @ 人）
#[derive(Debug, Clone, Serialize)]
pub struct TextMsg {
    msg_type: String,
    content: TextMsgContent,
}

#[derive(Debug, Clone, Serialize)]
struct TextMsgContent {
    text: String,
}

impl TextMsg {
    /// 创建文本消息
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            msg_type: "text".to_string(),
            content: TextMsgContent { text: content.into() },
        }
    }

    /// @ 指定用户（通过 open_id）
    pub fn at_user(mut self, open_id: impl Into<String>) -> Self {
        self.content.text = format!("{} <at user_id=\"{}\"></at>", self.content.text, open_id.into());
        self
    }

    /// @ 所有人
    pub fn at_all(mut self) -> Self {
        self.content.text = format!("{} <at user_id=\"all\"></at>", self.content.text);
        self
    }
}

/// 飞书卡片消息
#[derive(Debug, Clone, Serialize)]
pub struct CardMsg {
    msg_type: String,
    card: CardContent,
}

#[derive(Debug, Clone, Serialize)]
struct CardContent {
    config: CardConfig,
    header: CardHeader,
    elements: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct CardConfig {
    wide_screen_mode: bool,
}

#[derive(Debug, Clone, Serialize)]
struct CardHeader {
    title: CardTitle,
    #[serde(skip_serializing_if = "Option::is_none")]
    template: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CardTitle {
    tag: String,
    content: String,
}

impl CardMsg {
    /// 创建卡片消息
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            msg_type: "interactive".to_string(),
            card: CardContent {
                config: CardConfig { wide_screen_mode: true },
                header: CardHeader {
                    title: CardTitle {
                        tag: "plain_text".to_string(),
                        content: title.into(),
                    },
                    template: None,
                },
                elements: Vec::new(),
            },
        }
    }

    /// 设置卡片颜色主题（blue/green/red/orange）
    pub fn template(mut self, template: impl Into<String>) -> Self {
        self.card.header.template = Some(template.into());
        self
    }

    /// 添加 Markdown 内容块
    pub fn add_markdown(mut self, content: impl Into<String>) -> Self {
        self.card.elements.push(serde_json::json!({
            "tag": "markdown",
            "content": content.into()
        }));
        self
    }

    /// 添加分割线
    pub fn add_divider(mut self) -> Self {
        self.card.elements.push(serde_json::json!({ "tag": "hr" }));
        self
    }

    /// 添加普通文本块
    pub fn add_text(mut self, content: impl Into<String>) -> Self {
        self.card.elements.push(serde_json::json!({
            "tag": "div",
            "text": { "tag": "plain_text", "content": content.into() }
        }));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_msg() {
        let msg = TextMsg::new("hello").at_all();
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"msg_type\":\"text\""));
    }

    #[test]
    fn test_card_msg() {
        let msg = CardMsg::new("Title").template("red").add_markdown("**bold**");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"msg_type\":\"interactive\""));
    }
}
