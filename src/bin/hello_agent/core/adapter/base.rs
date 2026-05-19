use async_trait::async_trait;
use std::collections::HashMap;

use crate::hello_agent::core::exceptions::HelloAgentsError;
use crate::hello_agent::core::response::{LLMResponse, LLMToolResponse, StreamStats};
use crate::hello_agent::core::message::Message;

/// 工具选择策略
#[derive(Debug, Clone)]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Specific { function_name: String },
}

impl ToolChoice {
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Auto => serde_json::json!("auto"),
            Self::None => serde_json::json!("none"),
            Self::Required => serde_json::json!("required"),
            Self::Specific { function_name } => serde_json::json!({
                "type": "function",
                "function": { "name": function_name }
            }),
        }
    }
}

impl Default for ToolChoice {
    fn default() -> Self {
        Self::Auto
    }
}

/// LLM 适配器基类 trait
#[async_trait]
pub trait BaseLLMAdapter: Send + Sync {
    /// 获取最后一次流式统计
    fn last_stats(&self) -> Option<StreamStats>;

    /// 判断是否为 thinking model
    fn is_thinking_model(&self, model_name: &str) -> bool {
        let lower = model_name.to_lowercase();
        ["reasoner", "o1", "o3", "thinking"]
            .iter()
            .any(|kw| lower.contains(kw))
    }

    /// 非流式调用
    async fn invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<LLMResponse, HelloAgentsError>;

    /// 流式调用
    async fn stream_invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<String>, HelloAgentsError>;

    /// 异步流式调用
    async fn astream_invoke(
        &self,
        messages: &[Message],
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<String>, HelloAgentsError> {
        self.stream_invoke(messages, temperature, max_tokens).await
    }

    /// 工具调用
    async fn invoke_with_tools(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
        tool_choice: &ToolChoice,
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<LLMToolResponse, HelloAgentsError>;
}