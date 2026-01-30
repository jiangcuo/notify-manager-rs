//! # 邮件渠道示例
//!
//! 展示 SMTP 邮件发送的各种用法。

use notify_manager_rs::{email, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // ========================================
    // 基础邮件发送
    // ========================================
    println!("=== Basic Email ===");
    let mail = "dsads@dasdasda";
    let password = "dasdasd121";
    let port = 465;
    let smtp = "smtp.feishu.cn";
    let config = email::Config::new(smtp)
        .port(port)
        .tls(true)
        .credentials(mail, password)
        .from("alert@example.com")
        .to("sdasd@das.dasda");

    let client = email::Client::new(config.clone())?;
    client.send(&Message::new("Server Alert", "CPU usage exceeded 90%")).await?;
    println!("Basic email config created");

    // ========================================
    // 多收件人 + 抄送
    // ========================================
    println!("\n=== Multiple Recipients ===");

    let config_multi = email::Config::new(smtp)
        .credentials(mail,password)
        .from("alert@example.com")
        .to("adsdas@das.dsa")
        .cc("das@asd.com");

    let client_multi = email::Client::new(config_multi)?;
    client_multi.send(&Message::new("Team Alert", "Production incident")).await?;
    println!("Multi-recipient email config created");

    // ========================================
    // HTML 邮件（Markdown 格式）
    // ========================================
    println!("\n=== HTML Email ===");

    let html_msg = Message::markdown(
        "Weekly Report",
        r#"<h1>Weekly Report</h1>
<p>Here is your weekly summary:</p>
<ul>
<li>Total requests: 1,234,567</li>
<li>Error rate: 0.01%</li>
<li>Uptime: 99.99%</li>
</ul>"#,
    );

    let client = email::Client::new(config.clone())?;
    client.send(&html_msg).await?;
    println!("HTML email message created");

    Ok(())
}
