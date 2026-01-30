//! # 多渠道管理器模块
//!
//! 提供 `Sender` 结构体，用于统一管理多个渠道并支持广播发送。
//!
//! ## 功能特性
//! - 注册多个渠道（支持同类型多实例）
//! - 并发广播到所有渠道
//! - 按名称发送到指定渠道
//! - 部分失败时返回详细错误信息
//!
//! ## 使用示例
//! ```rust,ignore
//! use notify_manager_rs::{Sender, Message, dingtalk};
//!
//! let sender = Sender::new()
//!     .add("运维群", dingtalk::Client::new(config1))
//!     .add("开发群", dingtalk::Client::new(config2));
//!
//! // 广播到所有渠道
//! sender.send_all(&msg).await?;
//!
//! // 发送到指定渠道
//! sender.send_to("运维群", &msg).await?;
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use futures::future::join_all;
use tracing::{debug, error, info};

use crate::channel::Channel;
use crate::error::{ChannelError, Error};
use crate::message::Message;

/// 多渠道发送管理器
///
/// 管理多个渠道实例，支持广播和定向发送。
pub struct Sender {
    /// 渠道映射（名称 -> 渠道实例）
    channels: HashMap<String, Arc<dyn Channel>>,
}

impl Default for Sender {
    fn default() -> Self {
        Self::new()
    }
}

impl Sender {
    /// 创建空的 Sender
    ///
    /// # 示例
    /// ```rust
    /// use notify_manager_rs::Sender;
    ///
    /// let sender = Sender::new();
    /// ```
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// 添加渠道
    ///
    /// # 调用流程
    /// 1. 将渠道包装为 Arc
    /// 2. 插入到 HashMap
    /// 3. 返回 self（链式调用）
    ///
    /// # 参数
    /// * `name` - 渠道名称（用于后续路由）
    /// * `channel` - 实现了 Channel trait 的实例
    ///
    /// # 示例
    /// ```rust,ignore
    /// use notify_manager_rs::{Sender, dingtalk};
    ///
    /// let sender = Sender::new()
    ///     .add("运维群", dingtalk::Client::new(config));
    /// ```
    pub fn add<C: Channel + 'static>(mut self, name: impl Into<String>, channel: C) -> Self {
        let name = name.into();
        debug!(channel_name = %name, "registering channel");
        self.channels.insert(name, Arc::new(channel));
        self
    }

    /// 获取已注册的渠道数量
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// 获取所有已注册的渠道名称
    pub fn channel_names(&self) -> Vec<&str> {
        self.channels.keys().map(|s| s.as_str()).collect()
    }

    /// 广播消息到所有渠道
    ///
    /// # 调用流程
    /// ```text
    /// send_all(&msg)
    ///     │
    ///     ├──► 并发创建所有渠道的 Future
    ///     │         │
    ///     │         ├──► channel_1.send(&msg)
    ///     │         ├──► channel_2.send(&msg)
    ///     │         └──► channel_n.send(&msg)
    ///     │
    ///     ├──► join_all() 等待所有完成
    ///     │
    ///     ├──► 收集结果，分类成功/失败
    ///     │
    ///     └──► 全部成功 → Ok(())
    ///          部分失败 → Err(Partial { succeeded, failed })
    /// ```
    ///
    /// # 参数
    /// * `message` - 要发送的消息
    ///
    /// # 返回
    /// * `Ok(())` - 全部发送成功
    /// * `Err(Error::Partial { .. })` - 部分发送失败
    ///
    /// # 示例
    /// ```rust,ignore
    /// match sender.send_all(&msg).await {
    ///     Ok(()) => println!("全部成功"),
    ///     Err(Error::Partial { succeeded, failed }) => {
    ///         println!("成功: {:?}", succeeded);
    ///         println!("失败: {:?}", failed);
    ///     }
    ///     Err(e) => println!("错误: {}", e),
    /// }
    /// ```
    pub async fn send_all(&self, message: &Message) -> Result<(), Error> {
        if self.channels.is_empty() {
            debug!("no channels registered, skipping broadcast");
            return Ok(());
        }

        info!(
            channel_count = self.channels.len(),
            "broadcasting message to all channels"
        );

        // 并发发送到所有渠道
        let futures = self.channels.iter().map(|(name, channel)| {
            let name = name.clone();
            let channel = Arc::clone(channel);
            let msg = message.clone();
            async move {
                let result = channel.send(&msg).await;
                (name, result)
            }
        });

        let results = join_all(futures).await;

        // 分类结果
        let mut succeeded = Vec::new();
        let mut failed = Vec::new();

        for (name, result) in results {
            match result {
                Ok(()) => {
                    debug!(channel = %name, "message sent successfully");
                    succeeded.push(name);
                }
                Err(e) => {
                    error!(channel = %name, error = %e, "message send failed");
                    failed.push((name, e));
                }
            }
        }

        if failed.is_empty() {
            info!(
                succeeded_count = succeeded.len(),
                "broadcast completed successfully"
            );
            Ok(())
        } else {
            Err(Error::Partial { succeeded, failed })
        }
    }

    /// 发送消息到指定渠道
    ///
    /// # 调用流程
    /// 1. 验证渠道名称是否存在
    /// 2. 获取渠道实例
    /// 3. 调用渠道的 send 方法
    /// 4. 处理发送结果，记录日志
    ///
    /// # 参数
    /// * `name` - 渠道名称
    /// * `message` - 要发送的消息
    ///
    /// # 返回
    /// * `Ok(())` - 发送成功
    /// * `Err(Error::ChannelNotFound)` - 渠道不存在
    /// * `Err(Error::Channel { .. })` - 渠道发送失败
    ///
    /// # 示例
    /// ```rust,ignore
    /// sender.send_to("运维群", &msg).await?;
    /// ```
    pub async fn send_to(&self, name: &str, message: &Message) -> Result<(), Error> {
        // 1. 验证渠道是否存在
        let channel = self
            .channels
            .get(name)
            .ok_or_else(|| Error::ChannelNotFound(name.to_string()))?;

        // 2. 发送消息
        debug!(channel = %name, "sending message to channel");

        channel.send(message).await.map_err(|e| Error::Channel {
            channel: name.to_string(),
            source: e,
        })?;

        // 3. 记录成功日志
        info!(channel = %name, "message sent successfully");
        Ok(())
    }

    /// 发送消息到多个指定渠道
    ///
    /// # 调用流程
    /// 1. 筛选存在的渠道
    /// 2. 并发发送到这些渠道
    /// 3. 收集结果并返回
    ///
    /// # 参数
    /// * `names` - 渠道名称列表
    /// * `message` - 要发送的消息
    ///
    /// # 返回
    /// * `Ok(())` - 全部发送成功
    /// * `Err(Error::Partial { .. })` - 部分发送失败
    /// * `Err(Error::ChannelNotFound)` - 某个渠道不存在
    ///
    /// # 示例
    /// ```rust,ignore
    /// sender.send_to_many(&["运维群", "告警邮件"], &msg).await?;
    /// ```
    pub async fn send_to_many(&self, names: &[&str], message: &Message) -> Result<(), Error> {
        // 检查所有渠道是否存在
        for name in names {
            if !self.channels.contains_key(*name) {
                return Err(Error::ChannelNotFound((*name).to_string()));
            }
        }

        info!(
            channel_count = names.len(),
            channels = ?names,
            "sending message to multiple channels"
        );

        // 并发发送
        let futures = names.iter().map(|name| {
            let channel = Arc::clone(self.channels.get(*name).unwrap());
            let name = (*name).to_string();
            let msg = message.clone();
            async move {
                let result = channel.send(&msg).await;
                (name, result)
            }
        });

        let results = join_all(futures).await;

        // 分类结果
        let mut succeeded = Vec::new();
        let mut failed: Vec<(String, ChannelError)> = Vec::new();

        for (name, result) in results {
            match result {
                Ok(()) => succeeded.push(name),
                Err(e) => failed.push((name, e)),
            }
        }

        if failed.is_empty() {
            Ok(())
        } else {
            Err(Error::Partial { succeeded, failed })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    /// 测试用的模拟渠道
    struct MockChannel {
        name: String,
        should_fail: bool,
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn name(&self) -> &str {
            &self.name
        }

        async fn send(&self, _message: &Message) -> Result<(), ChannelError> {
            if self.should_fail {
                Err(ChannelError::Other("mock error".into()))
            } else {
                Ok(())
            }
        }
    }

    /// 测试：创建空 Sender
    #[test]
    fn test_new_sender() {
        let sender = Sender::new();
        assert_eq!(sender.channel_count(), 0);
    }

    /// 测试：添加渠道
    #[test]
    fn test_add_channel() {
        let sender = Sender::new()
            .add(
                "test1",
                MockChannel {
                    name: "test1".into(),
                    should_fail: false,
                },
            )
            .add(
                "test2",
                MockChannel {
                    name: "test2".into(),
                    should_fail: false,
                },
            );

        assert_eq!(sender.channel_count(), 2);
        assert!(sender.channel_names().contains(&"test1"));
        assert!(sender.channel_names().contains(&"test2"));
    }

    /// 测试：发送到不存在的渠道
    #[tokio::test]
    async fn test_send_to_nonexistent() {
        let sender = Sender::new();
        let msg = Message::text("test");

        let result = sender.send_to("nonexistent", &msg).await;
        assert!(matches!(result, Err(Error::ChannelNotFound(_))));
    }

    /// 测试：成功发送
    #[tokio::test]
    async fn test_send_success() {
        let sender = Sender::new().add(
            "test",
            MockChannel {
                name: "test".into(),
                should_fail: false,
            },
        );

        let msg = Message::text("test");
        let result = sender.send_to("test", &msg).await;
        assert!(result.is_ok());
    }

    /// 测试：部分失败
    #[tokio::test]
    async fn test_partial_failure() {
        let sender = Sender::new()
            .add(
                "success",
                MockChannel {
                    name: "success".into(),
                    should_fail: false,
                },
            )
            .add(
                "failure",
                MockChannel {
                    name: "failure".into(),
                    should_fail: true,
                },
            );

        let msg = Message::text("test");
        let result = sender.send_all(&msg).await;

        match result {
            Err(Error::Partial { succeeded, failed }) => {
                assert_eq!(succeeded.len(), 1);
                assert_eq!(failed.len(), 1);
                assert!(succeeded.contains(&"success".to_string()));
            }
            _ => panic!("expected Partial error"),
        }
    }
}
