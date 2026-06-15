use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::llm_response::{LlmResponse, StreamChunk};
use crate::core::types::message::{Message, ToolDefinition};
use async_trait::async_trait;
use futures::stream::Stream;
use std::pin::Pin;

/// =================================
/// 大模型提供商Provider
/// =================================
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        tool_choice: Option<&str>,
    ) -> Result<LlmResponse, HelloAgentError>;

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<StreamChunk, HelloAgentError>> + Send>>,
        HelloAgentError,
    >;
}

