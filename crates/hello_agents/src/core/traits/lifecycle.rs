use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::types::event::AgentEvent;

/// 生命周期钩子类型
pub type LifecycleHook =
Arc<dyn Fn(AgentEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;