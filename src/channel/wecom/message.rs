//! # 企业微信原生消息类型
//!
//! 支持企业微信特有功能，如 @ 指定人员、图文消息等。

use serde::Serialize;

/// 企业微信文本消息（支持 @ 人）
#[derive(Debug, Clone, Serialize)]
pub struct TextMsg {
    msgtype: String,
    text: TextMsgContent,
}

#[derive(Debug, Clone, Serialize)]
struct TextMsgContent {
    content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mentioned_list: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mentioned_mobile_list: Vec<String>,
}

impl TextMsg {
    /// 创建文本消息
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            msgtype: "text".to_string(),
            text: TextMsgContent {
                content: content.into(),
                mentioned_list: Vec::new(),
                mentioned_mobile_list: Vec::new(),
            },
        }
    }

    /// @ 指定用户（通过 userid）
    pub fn mention(mut self, user_ids: Vec<impl Into<String>>) -> Self {
        self.text.mentioned_list = user_ids.into_iter().map(|u| u.into()).collect();
        self
    }

    /// @ 指定用户（通过手机号）
    pub fn mention_mobile(mut self, mobiles: Vec<impl Into<String>>) -> Self {
        self.text.mentioned_mobile_list = mobiles.into_iter().map(|m| m.into()).collect();
        self
    }

    /// @ 所有人
    pub fn mention_all(mut self) -> Self {
        self.text.mentioned_list.push("@all".to_string());
        self
    }
}

/// 企业微信 Markdown 消息
#[derive(Debug, Clone, Serialize)]
pub struct MarkdownMsg {
    msgtype: String,
    markdown: MarkdownContent,
}

#[derive(Debug, Clone, Serialize)]
struct MarkdownContent {
    content: String,
}

impl MarkdownMsg {
    /// 创建 Markdown 消息
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            msgtype: "markdown".to_string(),
            markdown: MarkdownContent {
                content: content.into(),
            },
        }
    }
}

/// 企业微信图文消息
#[derive(Debug, Clone, Serialize)]
pub struct NewsMsg {
    msgtype: String,
    news: NewsContent,
}

#[derive(Debug, Clone, Serialize)]
struct NewsContent {
    articles: Vec<Article>,
}

#[derive(Debug, Clone, Serialize)]
struct Article {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    picurl: Option<String>,
}

impl NewsMsg {
    /// 创建图文消息
    pub fn new() -> Self {
        Self {
            msgtype: "news".to_string(),
            news: NewsContent {
                articles: Vec::new(),
            },
        }
    }

    /// 添加图文条目
    pub fn add_article(
        mut self,
        title: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        self.news.articles.push(Article {
            title: title.into(),
            description: None,
            url: url.into(),
            picurl: None,
        });
        self
    }

    /// 添加完整图文条目
    pub fn add_article_full(
        mut self,
        title: impl Into<String>,
        description: impl Into<String>,
        url: impl Into<String>,
        picurl: impl Into<String>,
    ) -> Self {
        self.news.articles.push(Article {
            title: title.into(),
            description: Some(description.into()),
            url: url.into(),
            picurl: Some(picurl.into()),
        });
        self
    }
}

impl Default for NewsMsg {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_msg() {
        let msg = TextMsg::new("hello").mention_all();
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"msgtype\":\"text\""));
        assert!(json.contains("@all"));
    }

    #[test]
    fn test_markdown_msg() {
        let msg = MarkdownMsg::new("**bold**");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"msgtype\":\"markdown\""));
    }

    #[test]
    fn test_news_msg() {
        let msg = NewsMsg::new().add_article("Title", "https://example.com");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"msgtype\":\"news\""));
    }
}
