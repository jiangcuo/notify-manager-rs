//! # 飞书渠道示例
//!
//! 展示飞书机器人的各种用法。

use notify_manager_rs::feishu::{self, CardMsg, Client, Config, TextMsg};
use notify_manager_rs::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // ========================================
    // 基础用法
    // ========================================
    println!("=== Basic Feishu Usage ===");

    let config = Config::new("https://open.feishu.cn/open-apis/bot/v2/hook/xxxxxxxx")
        .secret("sssssssssss");

    let baseclient = Client::new(config.clone());

    baseclient.send(&Message::text("服务器告警")).await?;
    println!("Basic config created");

    // ========================================
    // 文本消息 + @ 人
    // ========================================
    println!("\n=== Text Message with @mentions ===");

    let _text_msg = TextMsg::new("Server alert: CPU usage exceeded 90%")
        .at_user("ou_xxx")
        .at_all();

    // client.send_native(&text_msg).await?;
    println!("TextMsg with @mentions created");

    // ========================================
    // 卡片消息
    // ========================================
    println!("\n=== Card Message ===");

    let card_msg = CardMsg::new("Production Alert")
        .template("red")
        .add_markdown("**Host**: server-01")
        .add_markdown("**CPU**: 95%")
        .add_divider()
        .add_text("Please check immediately!");

    baseclient.send_native(&card_msg).await?;
    println!("CardMsg created");

    println!("\nAll demos completed (skipped actual sending)");
    Ok(())
}
