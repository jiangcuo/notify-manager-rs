//! # 企业微信渠道示例
//!
//! 展示企业微信机器人的各种用法。

use notify_manager_rs::wecom::{self, Client, Config, MarkdownMsg, NewsMsg, TextMsg};
use notify_manager_rs::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // ========================================
    // 基础用法
    // ========================================
    println!("=== Basic WeCom Usage ===");

    let config = Config::new("https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxxx");

    let client = Client::new(config.clone());

    client.send(&Message::text("服务器告警")).await?;
    println!("Basic config created");

    // ========================================
    // 文本消息 + @ 人
    // ========================================
    println!("\n=== Text Message with @mentions ===");

    let text_msg = TextMsg::new("Server alert: CPU usage exceeded 90%")
        .mention(vec!["userid1", "userid2"])
        .mention_mobile(vec!["13800138000"]);

    client.send_native(&text_msg).await?;
    println!("TextMsg with @mentions created");

    // @ 所有人
    let text_all = TextMsg::new("Urgent: Production down!").mention_all();
    println!("TextMsg with @all created");

    // ========================================
    // Markdown 消息
    // ========================================
    println!("\n=== Markdown Message ===");

    let markdown_msg = MarkdownMsg::new(
        r#"## Server Alert
> Host: **server-01**
> CPU: <font color="warning">95%</font>
> Memory: <font color="info">60%</font>

Please check [dashboard](https://example.com)"#,
    );

    // client.send_native(&markdown_msg).await?;
    println!("MarkdownMsg created");

    // ========================================
    // 图文消息
    // ========================================
    println!("\n=== News Message ===");

    let news_msg = NewsMsg::new()
        .add_article("Alert: Server Down", "https://example.com/alert/1")
        .add_article_full(
            "Weekly Report",
            "Click to view the weekly system report",
            "https://example.com/report",
            "https://example.com/images/report.png",
        );

    client.send_native(&news_msg).await?;
    println!("NewsMsg created");

    // ========================================
    // 与 Sender 集成
    // ========================================
    println!("\n=== Integration with Sender ===");

    use notify_manager_rs::Sender;

    let config1 = Config::new("WEBHOOK_URL_1");
    let config2 = Config::new("WEBHOOK_URL_2");

    let _sender = Sender::new()
        .add("wecom-ops", Client::new(config1))
        .add("wecom-dev", Client::new(config2));

    // sender.send_all(&Message::new("Alert", "Server down")).await?;
    println!("Sender with multiple WeCom channels created");

    println!("\nAll demos completed (skipped actual sending)");
    Ok(())
}
