//! # Webhook 渠道示例
//!
//! 展示通用 Webhook 的各种用法。

use notify_manager_rs::{webhook, Message};
use reqwest::Method;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // ========================================
    // 基础 POST JSON
    // ========================================
    println!("=== Basic POST JSON ===");

    let config = webhook::Config::new("https://example.com/webhook")
        .header("X-Custom-Header", "custom-value")
        .timeout(Duration::from_secs(5));

    let client = webhook::Client::new(config);
    // client.send(&Message::text("Hello from notify-manager-rs")).await?;
    println!("Basic webhook created");

    // ========================================
    // Bearer Token 认证
    // ========================================
    println!("\n=== Bearer Token Auth ===");

    let config_with_auth = webhook::Config::new("https://api.example.com/notify")
        .bearer_auth("your-api-token")
        .retry(5, Duration::from_millis(100));

    let client_with_auth = webhook::Client::new(config_with_auth);
    // client_with_auth.send(&Message::new("Alert", "Server down")).await?;
    println!("Webhook with bearer auth created");

    // ========================================
    // Basic 认证
    // ========================================
    println!("\n=== Basic Auth ===");

    let config_basic = webhook::Config::new("https://internal.example.com/alert")
        .basic_auth("username", "password")
        .no_retry();

    let client_basic = webhook::Client::new(config_basic);
    // client_basic.send(&Message::text("Internal alert")).await?;
    println!("Webhook with basic auth created");

    // ========================================
    // 自定义 JSON 数据
    // ========================================
    println!("\n=== Custom JSON Payload ===");

    #[derive(serde::Serialize)]
    struct SlackMessage {
        text: String,
        channel: String,
        username: String,
    }

    let slack_config = webhook::Config::new("https://hooks.slack.com/services/xxx")
        .header("Content-Type", "application/json");

    let slack_client = webhook::Client::new(slack_config);

    let slack_msg = SlackMessage {
        text: "Hello from notify-manager-rs!".to_string(),
        channel: "#alerts".to_string(),
        username: "notify-bot".to_string(),
    };

    // slack_client.send_json(&slack_msg).await?;
    println!("Custom JSON payload created");

    println!("\nAll demos completed (skipped actual sending)");
    Ok(())
}
