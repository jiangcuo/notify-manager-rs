//! # 错误类型模块
//!
//! 定义库级别和渠道级别的错误类型。
//!
//! ## 错误层级
//! - `Error` - 库级别错误，用于对外 API
//! - `ChannelError` - 渠道级别错误，区分可重试/不可重试

use std::time::Duration;
use thiserror::Error;

/// 库级别错误
///
/// 用于所有公开 API 的返回类型。
#[derive(Debug, Error)]
pub enum Error {
    /// 单渠道发送失败
    #[error("channel '{channel}' failed: {source}")]
    Channel {
        /// 渠道名称
        channel: String,
        /// 具体错误
        source: ChannelError,
    },

    /// 多渠道部分失败（广播时）
    #[error("partial failure: {} succeeded, {} failed", succeeded.len(), failed.len())]
    Partial {
        /// 发送成功的渠道名称列表
        succeeded: Vec<String>,
        /// 发送失败的渠道及错误
        failed: Vec<(String, ChannelError)>,
    },

    /// 渠道不存在
    #[error("channel not found: {0}")]
    ChannelNotFound(String),
}

/// 渠道级别错误
///
/// 区分可重试和不可重试错误，便于上层实现重试逻辑。
#[derive(Debug, Error)]
pub enum ChannelError {
    // ========== 可重试错误 ==========
    /// 网络请求失败
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// 请求超时
    #[error("request timeout")]
    Timeout,

    /// 超出频率限制
    #[error("rate limit exceeded, retry after {retry_after:?}")]
    RateLimit {
        /// 建议的重试等待时间
        retry_after: Option<Duration>,
    },

    // ========== 不可重试错误 ==========
    /// 认证失败
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// 消息格式错误
    #[error("invalid message: {0}")]
    InvalidMessage(String),

    /// 服务端返回错误
    #[error("server error: code={code}, message={message}")]
    ServerError {
        /// 错误码
        code: i32,
        /// 错误消息
        message: String,
    },

    /// 邮件发送失败
    #[error("email error: {0}")]
    Email(String),

    /// 其他错误
    #[error("{0}")]
    Other(String),
}

impl ChannelError {
    /// 判断错误是否可重试
    ///
    /// # 返回
    /// - `true` - 可重试（网络错误、超时、限流）
    /// - `false` - 不可重试（认证失败、消息格式错误等）
    ///
    /// # 示例
    /// ```rust
    /// use notify_manager_rs::ChannelError;
    ///
    /// let err = ChannelError::Timeout;
    /// assert!(err.is_retryable());
    ///
    /// let err = ChannelError::Unauthorized("invalid token".into());
    /// assert!(!err.is_retryable());
    /// ```
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Timeout | Self::RateLimit { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：可重试错误判断
    #[test]
    fn test_is_retryable() {
        // 可重试
        assert!(ChannelError::Timeout.is_retryable());
        assert!(ChannelError::RateLimit { retry_after: None }.is_retryable());

        // 不可重试
        assert!(!ChannelError::Unauthorized("test".into()).is_retryable());
        assert!(!ChannelError::InvalidMessage("test".into()).is_retryable());
        assert!(!ChannelError::ServerError {
            code: 500,
            message: "test".into()
        }
        .is_retryable());
    }
}
