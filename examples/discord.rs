//! Discord 使用示例

use notify_manager_rs::{discord, ChannelError, Message, Sender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // === 基础用法 ===
    println!("=== Basic Discord Usage ===");

    let config = discord::Config::new("https://discord.com/api/webhooks/YOUR/WEBHOOK")
        .username("AlertBot")
        .avatar_url("https://example.com/bot-avatar.png");

    let client = discord::Client::new(config);

    // 发送简单消息
    client.send(&Message::text("Hello from Rust!")).await?;

    // === 带 Embed 的消息 ===
    println!("\n=== Message with Embed ===");

    let msg = Message::new("🚨 服务告警", "数据库连接池耗尽，当前活跃连接数: 100")
        .with_level(notify_manager_rs::Level::Error);

    client.send(&msg).await?;

    // === 不同级别的消息 ===
    println!("\n=== Different Alert Levels ===");

    let info_msg = Message::new("Info", "服务已启动").with_level(notify_manager_rs::Level::Info);
    let warn_msg = Message::new("Warning", "内存使用率 85%").with_level(notify_manager_rs::Level::Warning);
    let error_msg = Message::new("Error", "API 响应超时").with_level(notify_manager_rs::Level::Error);
    let critical_msg = Message::new("Critical", "服务不可用").with_level(notify_manager_rs::Level::Critical);

    client.send(&info_msg).await?;
    client.send(&warn_msg).await?;
    client.send(&error_msg).await?;
    client.send(&critical_msg).await?;

    // === 多渠道集成 ===
    println!("\n=== Multi-channel with Sender ===");

    let config1 = discord::Config::new("https://discord.com/api/webhooks/SERVER1");
    let config2 = discord::Config::new("https://discord.com/api/webhooks/SERVER2");

    let sender = Sender::new()
        .add("gaming-server", discord::Client::new(config1))
        .add("work-server", discord::Client::new(config2));

    sender
        .send_all(&Message::text("Broadcast to all servers"))
        .await?;

    Ok(())
}
