//! # 通用 Webhook 渠道模块
//!
//! 提供灵活的 HTTP Webhook 消息推送功能，支持自定义请求格式。
//!
//! ## 功能特性
//! - 支持 GET/POST 方法
//! - 支持自定义 Headers
//! - 支持自定义请求体模板
//! - 支持 Basic Auth / Bearer Token 认证
//! - 超时和重试机制
//!
//! ## 调用流程
//! ```text
//! Config::new() → Client::new() → client.send() → HTTP Request → 解析响应
//!                                       ↓
//!                              失败时触发重试逻辑
//! ```
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::webhook;
//! use notify_manager_rs::Message;
//!
//! // 简单 POST JSON
//! let config = webhook::Config::new("https://example.com/webhook")
//!     .header("X-Custom-Header", "value");
//!
//! let client = webhook::Client::new(config);
//! client.send(&Message::text("告警")).await?;
//! ```

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client as HttpClient, Method};
use serde::Serialize;
use tracing::{debug, error, info, warn};

use crate::channel::Channel;
use crate::error::ChannelError;
use crate::message::Message;

/// 默认超时时间
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// 默认重试次数
const DEFAULT_RETRY_ATTEMPTS: u32 = 3;

/// 默认重试基础延迟
const DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(200);

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 基础延迟时间
    pub base_delay: Duration,
    /// 最大延迟时间
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_RETRY_ATTEMPTS,
            base_delay: DEFAULT_RETRY_DELAY,
            max_delay: Duration::from_secs(5),
        }
    }
}

/// 认证方式
#[derive(Debug, Clone)]
pub enum Auth {
    /// Basic 认证
    Basic { username: String, password: String },
    /// Bearer Token 认证
    Bearer(String),
    /// 自定义 Header 认证
    Header { name: String, value: String },
}

/// Webhook 渠道配置
#[derive(Debug, Clone)]
pub struct Config {
    /// Webhook URL
    url: String,
    /// HTTP 方法
    method: Method,
    /// 自定义 Headers
    headers: HashMap<String, String>,
    /// 认证方式
    auth: Option<Auth>,
    /// 请求超时时间
    timeout: Duration,
    /// 重试配置
    retry: Option<RetryConfig>,
    /// 内容类型
    content_type: String,
}

impl Config {
    /// 创建 Webhook 配置
    ///
    /// 默认使用 POST 方法和 JSON 格式。
    ///
    /// # 参数
    /// * `url` - Webhook URL
    ///
    /// # 示例
    /// ```rust
    /// use notify_manager_rs::webhook::Config;
    ///
    /// let config = Config::new("https://example.com/webhook");
    /// ```
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: Method::POST,
            headers: HashMap::new(),
            auth: None,
            timeout: DEFAULT_TIMEOUT,
            retry: Some(RetryConfig::default()),
            content_type: "application/json".to_string(),
        }
    }

    /// 设置 HTTP 方法
    ///
    /// # 参数
    /// * `method` - HTTP 方法（GET/POST/PUT 等）
    pub fn method(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    /// 添加自定义 Header
    ///
    /// # 参数
    /// * `name` - Header 名称
    /// * `value` - Header 值
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// 设置 Basic 认证
    ///
    /// # 参数
    /// * `username` - 用户名
    /// * `password` - 密码
    pub fn basic_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.auth = Some(Auth::Basic {
            username: username.into(),
            password: password.into(),
        });
        self
    }

    /// 设置 Bearer Token 认证
    ///
    /// # 参数
    /// * `token` - Bearer Token
    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.auth = Some(Auth::Bearer(token.into()));
        self
    }

    /// 设置请求超时时间
    ///
    /// # 参数
    /// * `timeout` - 超时时间
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 设置重试策略
    ///
    /// # 参数
    /// * `max_attempts` - 最大重试次数
    /// * `base_delay` - 基础延迟时间
    pub fn retry(mut self, max_attempts: u32, base_delay: Duration) -> Self {
        self.retry = Some(RetryConfig {
            max_attempts,
            base_delay,
            max_delay: Duration::from_secs(5),
        });
        self
    }

    /// 禁用重试
    pub fn no_retry(mut self) -> Self {
        self.retry = None;
        self
    }

    /// 设置内容类型
    ///
    /// # 参数
    /// * `content_type` - Content-Type 值
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = content_type.into();
        self
    }
}

/// Webhook 客户端
///
/// 持有 HTTP 连接池，适合频繁发送场景。
pub struct Client {
    /// 配置
    config: Config,
    /// HTTP 客户端
    http: HttpClient,
    /// 渠道名称
    name: String,
}

impl Client {
    /// 创建 Webhook 客户端
    ///
    /// # 调用流程
    /// 1. 使用配置创建 HTTP 客户端
    /// 2. 设置超时时间
    /// 3. 返回客户端实例
    ///
    /// # 参数
    /// * `config` - Webhook 配置
    pub fn new(config: Config) -> Self {
        let http = HttpClient::builder()
            .timeout(config.timeout)
            .build()
            .expect("failed to create http client");

        Self {
            config,
            http,
            name: "webhook".to_string(),
        }
    }

    /// 设置渠道名称
    ///
    /// 用于在 Sender 中区分多个 Webhook 渠道。
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 发送通用消息
    ///
    /// # 调用流程
    /// 1. 将 Message 转换为 JSON
    /// 2. 构建 HTTP 请求（添加 Headers、认证）
    /// 3. 发送请求
    /// 4. 解析响应状态
    /// 5. 失败时根据配置进行重试
    ///
    /// # 参数
    /// * `message` - 通用消息
    ///
    /// # 返回
    /// * `Ok(())` - 发送成功
    /// * `Err(ChannelError)` - 发送失败
    pub async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        self.send_json(message).await
    }

    /// 发送自定义 JSON 数据
    ///
    /// # 参数
    /// * `body` - 任意可序列化的数据
    pub async fn send_json<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        match &self.config.retry {
            Some(retry_config) => self.send_with_retry(body, retry_config).await,
            None => self.do_send(body).await,
        }
    }

    /// 带重试的发送
    async fn send_with_retry<T: Serialize>(
        &self,
        body: &T,
        retry_config: &RetryConfig,
    ) -> Result<(), ChannelError> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < retry_config.max_attempts {
            attempts += 1;

            match self.do_send(body).await {
                Ok(()) => {
                    if attempts > 1 {
                        info!(attempts, "webhook message sent after retry");
                    }
                    return Ok(());
                }
                Err(e) => {
                    if !e.is_retryable() {
                        return Err(e);
                    }

                    last_error = Some(e);

                    if attempts < retry_config.max_attempts {
                        let delay = std::cmp::min(
                            retry_config.base_delay * 2u32.pow(attempts - 1),
                            retry_config.max_delay,
                        );
                        warn!(
                            attempt = attempts,
                            max_attempts = retry_config.max_attempts,
                            delay_ms = delay.as_millis(),
                            "webhook send failed, retrying"
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ChannelError::Other("unknown error".into())))
    }

    /// 执行单次发送
    async fn do_send<T: Serialize>(&self, body: &T) -> Result<(), ChannelError> {
        let mut request = self
            .http
            .request(self.config.method.clone(), &self.config.url)
            .header("Content-Type", &self.config.content_type);

        // 添加自定义 Headers
        for (name, value) in &self.config.headers {
            request = request.header(name, value);
        }

        // 添加认证
        request = match &self.config.auth {
            Some(Auth::Basic { username, password }) => {
                request.basic_auth(username, Some(password))
            }
            Some(Auth::Bearer(token)) => request.bearer_auth(token),
            Some(Auth::Header { name, value }) => request.header(name, value),
            None => request,
        };

        // 设置请求体
        if self.config.method != Method::GET {
            request = request.json(body);
        }

        debug!(url = %self.config.url, method = %self.config.method, "sending webhook request");

        let response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                ChannelError::Timeout
            } else {
                ChannelError::Network(e)
            }
        })?;

        let status = response.status();

        debug!(status = %status, "webhook response");

        if status.is_success() {
            info!("webhook message sent successfully");
            Ok(())
        } else if status.as_u16() == 429 {
            // Rate limit
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(Duration::from_secs);

            Err(ChannelError::RateLimit { retry_after })
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            let body = response.text().await.unwrap_or_default();
            error!(status = %status, body = %body, "webhook authentication failed");
            Err(ChannelError::Unauthorized(body))
        } else {
            let body = response.text().await.unwrap_or_default();
            error!(status = %status, body = %body, "webhook request failed");
            Err(ChannelError::ServerError {
                code: status.as_u16() as i32,
                message: body,
            })
        }
    }
}

#[async_trait]
impl Channel for Client {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, message: &Message) -> Result<(), ChannelError> {
        Client::send(self, message).await
    }
}

/// 一次性发送函数
///
/// 适合脚本或一次性任务。
///
/// # 参数
/// * `config` - Webhook 配置
/// * `message` - 要发送的消息
///
/// # 示例
/// ```rust,ignore
/// use notify_manager_rs::webhook::{self, Config};
/// use notify_manager_rs::Message;
///
/// webhook::send(
///     &Config::new("https://example.com/webhook"),
///     &Message::text("告警")
/// ).await?;
/// ```
pub async fn send(config: &Config, message: &Message) -> Result<(), ChannelError> {
    let client = Client::new(config.clone());
    client.send(message).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：配置创建
    #[test]
    fn test_config_builder() {
        let config = Config::new("https://example.com")
            .method(Method::PUT)
            .header("X-Custom", "value")
            .bearer_auth("token123")
            .timeout(Duration::from_secs(5));

        assert_eq!(config.url, "https://example.com");
        assert_eq!(config.method, Method::PUT);
        assert_eq!(config.headers.get("X-Custom"), Some(&"value".to_string()));
        assert!(matches!(config.auth, Some(Auth::Bearer(_))));
    }

    /// 测试：Basic 认证
    #[test]
    fn test_basic_auth() {
        let config = Config::new("url").basic_auth("user", "pass");
        assert!(matches!(
            config.auth,
            Some(Auth::Basic {
                username,
                password
            }) if username == "user" && password == "pass"
        ));
    }
}
