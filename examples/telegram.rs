//! Telegram 使用示例

use notify_manager_rs::{telegram, Message, Sender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // === 基础用法 ===
    println!("=== Basic Telegram Usage ===");

    // 从 @BotFather 获取 bot token
    // chat_id 可以是用户 ID、群组 ID 或频道 @username
    let config = telegram::Config::new("123456789:ABCdefGHIjklMNOpqrsTUVwxyz", "-1001234567890");

    let client = telegram::Client::new(config);

    // 发送简单消息
    client.send(&Message::text("Hello from Rust!")).await?;

    // === 带标题的消息 ===
    println!("\n=== Message with Title ===");

    let msg = Message::new("🚨 服务告警", "CPU 使用率超过 90%\n请及时处理！");

    client.send(&msg).await?;

    // === 静默消息 ===
    println!("\n=== Silent Notification ===");

    let silent_config = telegram::Config::new("BOT_TOKEN", "CHAT_ID").silent();

    let silent_client = telegram::Client::new(silent_config);
    silent_client
        .send(&Message::text("This won't trigger a notification sound"))
        .await?;

    // === MarkdownV2 格式 ===
    println!("\n=== MarkdownV2 Format ===");

    let md_config = telegram::Config::new("BOT_TOKEN", "CHAT_ID").parse_mode("MarkdownV2");

    let md_client = telegram::Client::new(md_config);
    // 注意：MarkdownV2 需要手动转义特殊字符
    md_client
        .send(&Message::text("*Bold* _italic_ `code`"))
        .await?;

    // === 多渠道集成 ===
    println!("\n=== Multi-channel with Sender ===");

    let config1 = telegram::Config::new("BOT_TOKEN", "GROUP_CHAT_ID");
    let config2 = telegram::Config::new("BOT_TOKEN", "CHANNEL_ID");

    let sender = Sender::new()
        .add("ops-group", telegram::Client::new(config1))
        .add("public-channel", telegram::Client::new(config2));

    sender
        .send_all(&Message::text("Broadcast to all chats"))
        .await?;

    Ok(())
}
