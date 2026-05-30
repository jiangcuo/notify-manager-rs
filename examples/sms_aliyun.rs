//! # 阿里云短信渠道示例
//!
//! 展示三层 API 用法。运行前请替换为你自己的阿里云配置：
//! - AccessKey ID / Secret（建议从环境变量读取，避免硬编码）
//! - 短信签名 SignName
//! - 短信模板 TemplateCode
//!
//! ```bash
//! export ALIYUN_AK_ID=LTAI_xxx
//! export ALIYUN_AK_SECRET=xxx
//! cargo run --example sms_aliyun
//! ```

use notify_manager_rs::sms::aliyun;
use notify_manager_rs::{Message, Sender};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 从环境变量读取密钥（不要把密钥写进源码）
    let ak_id = std::env::var("ALIYUN_AK_ID").unwrap_or_else(|_| "dsad".into());
    let ak_secret =
        std::env::var("ALIYUN_AK_SECRET").unwrap_or_else(|_| "dasdasd".into());

    // ========================================
    // Layer 1：一次性发送
    // ========================================
    println!("=== Layer 1: One-shot Send ===");

    let config = aliyun::Config::new(&ak_id, &ak_secret)
        .region("cn-hangzhou")
        .sign_name("dsada")
        .template_code("SMS_507175041")
        .to("dasdasdas");

    // 模板形如：您的验证码是 ${code}，请勿泄露。
    // extra 中的键值对会拼成模板参数：{"code":"8888"}
    let msg = Message::builder()
        .content("验证码通知") // 仅用于日志，不参与发送
        .extra("code", "8888")
        .build();

    // 未配置真实密钥时这里会返回错误，属正常现象
    match aliyun::send(&config, &msg).await {
        Ok(()) => println!("发送成功"),
        Err(e) => println!("发送失败（示例未配置真实密钥时正常）: {}", e),
    }

    Ok(())
}
