//! # 钉钉原生消息示例
//!
//! 展示钉钉特有功能，如 @ 指定人员。

use notify_manager_rs::dingtalk::{self, Client, Config, MarkdownMsg, TextMsg};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = Config::new("YOUR_WEBHOOK_URL")
        .secret("YOUR_SECRET")
        .retry(3, std::time::Duration::from_millis(500));

    let client = Client::new(config);

    // ========================================
    // 文本消息 + @ 指定人员
    // ========================================
    let text_msg = TextMsg::new("Server alert: CPU usage exceeded 90%")
        .at_mobiles(vec!["13800138000", "13900139000"])
        .at_all(false);

    // client.send_native(&text_msg).await?;
    println!("TextMsg with @mentions created");

    // ========================================
    // Markdown 消息 + @ 所有人
    // ========================================
    let markdown_msg = MarkdownMsg::new(
        "Production Alert",
        r#"## Server Alert
        
**Host**: server-01
**CPU**: 95%
**Memory**: 80%

Please check immediately!"#,
    )
    .at_all(true);

    // client.send_native(&markdown_msg).await?;
    println!("MarkdownMsg with @all created");

    // ========================================
    // Markdown + @ 指定用户 ID
    // ========================================
    let msg_with_user_ids = MarkdownMsg::new("Deploy Notice", "Deployment completed successfully")
        .at_user_ids(vec!["user123", "user456"]);

    // client.send_native(&msg_with_user_ids).await?;
    println!("MarkdownMsg with @userIds created");

    println!("\nAll demos completed (skipped actual sending without real webhook URL)");
    Ok(())
}
