//! # 基础用法示例
//!
//! 展示 notify-manager-rs 的三层 API 用法。

use notify_manager_rs::{dingtalk, Message, Sender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 tracing（可选，用于查看日志）
    tracing_subscriber::fmt::init();

    // ========================================
    // Layer 1: 一次性发送（最简用法）
    // ========================================
    println!("=== Layer 1: One-shot Send ===");

    let config = dingtalk::Config::new("YOUR_WEBHOOK_URL")
        .secret("YOUR_SECRET");

    // dingtalk::send(&config, &Message::text("服务器告警")).await?;
    println!("Layer 1 demo (skipped without real webhook URL)");

    // ========================================
    // Layer 2: 复用连接（单渠道频繁发送）
    // ========================================
    println!("\n=== Layer 2: Reuse Connection ===");

    let client = dingtalk::Client::new(config.clone());

    // client.send(&Message::text("告警 1")).await?;
    // client.send(&Message::text("告警 2")).await?;
    println!("Layer 2 demo (skipped without real webhook URL)");

    // ========================================
    // Layer 3: 多渠道管理
    // ========================================
    println!("\n=== Layer 3: Multi-channel Sender ===");

    let config1 = dingtalk::Config::new("WEBHOOK_URL_1");
    let config2 = dingtalk::Config::new("WEBHOOK_URL_2");

    let sender = Sender::new()
        .add("ops-team", dingtalk::Client::new(config1))
        .add("dev-team", dingtalk::Client::new(config2));

    println!("Registered channels: {:?}", sender.channel_names());

    let msg = Message::builder()
        .title("CPU Alert")
        .content("CPU usage exceeded 90%")
        .level(notify_manager_rs::Level::Warning)
        .build();

    // 广播到所有渠道
    // sender.send_all(&msg).await?;

    // 发送到指定渠道
    // sender.send_to("ops-team", &msg).await?;

    println!("Layer 3 demo (skipped without real webhook URL)");

    Ok(())
}
