use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::hello_agent::core::config::Config;
use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::llm::HelloAgentsLLM;
use crate::hello_agent::core::message::Message;
use crate::hello_agent::core::lifecycle::{AgentEvent, EventType, LifecycleHook, ExecutionContext};
use crate::hello_agent::core::streaming::StreamEvent;

/// Agent 基类 trait
#[async_trait]
pub trait Agent: Send + Sync {
    /// Agent 名称
    fn name(&self) -> &str;

    /// LLM 实例
    fn llm(&self) -> &HelloAgentsLLM;

    /// 系统提示词
    fn system_prompt(&self) -> Option<&str>;

    /// 配置
    fn config(&self) -> Option<&Config>;

    /// 运行 Agent（同步版本）
    async fn run(&mut self, input_text: &str) -> Result<String, HelloAgentsError>;

    /// 流式运行
    async fn run_stream(
        &mut self,
        input_text: &str,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent>, HelloAgentsError>;

    /// 异步执行
    async fn arun(
        &mut self,
        input_text: &str,
        on_start: Option<LifecycleHook>,
        on_step: Option<LifecycleHook>,
        on_finish: Option<LifecycleHook>,
        on_error: Option<LifecycleHook>,
    ) -> Result<String, HelloAgentsError>;

    /// 获取历史
    fn get_history(&self) -> Vec<Message>;

    /// 清空历史
    fn clear_history(&mut self);
}