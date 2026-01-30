//! Slack 使用示例

use notify_manager_rs::{slack, Message, Sender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // === 基础用法 ===
    println!("=== Basic Slack Usage ===");

    let config = slack::Config::new("https://hooks.slack.com/services/YOUR/WEBHOOK/URL")
        .username("AlertBot")
        .icon_emoji(":robot_face:")
        .channel("#alerts");

    let client = slack::Client::new(config);

    // 发送简单消息
    client.send(&Message::text("Hello from Rust!")).await?;

    // === 带标题的消息 ===
    println!("\n=== Message with Title ===");

    let msg = Message::new("服务告警", "CPU 使用率超过 90%")
        .with_level(notify_manager_rs::Level::Warning);

    client.send(&msg).await?;

    // === 多渠道集成 ===
    println!("\n=== Multi-channel with Sender ===");

    let config1 = slack::Config::new("https://hooks.slack.com/services/CHANNEL1");
    let config2 = slack::Config::new("https://hooks.slack.com/services/CHANNEL2");

    let sender = Sender::new()
        .add("ops-alerts", slack::Client::new(config1))
        .add("dev-alerts", slack::Client::new(config2));

    // 广播到所有渠道
    sender.send_all(&Message::text("Broadcast message")).await?;

    // 发送到指定渠道
    sender
        .send_to("ops-alerts", &Message::text("Ops only"))
        .await?;

    Ok(())
}
