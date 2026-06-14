use std::collections::HashMap;
use std::pin::Pin;

use async_trait::async_trait;
use futures::stream::Stream;

use crate::core::types::exceptions::HelloAgentException;
use crate::core::types::llm_response::{LlmResponse, LlmToolResponse, StreamStats};
use crate::core::types::message::Message;

#[async_trait]
pub trait LLMAdapter: Send + Sync {
    fn create_sync_client(&self) -> Result<reqwest::blocking::Client, HelloAgentException>;
    fn create_async_client(&self) -> Result<reqwest::Client, HelloAgentException>;

    fn invoke(
        &self,
        messages: Vec<Message>,
        kwargs: HashMap<String, String>,
    ) -> Result<LlmResponse, HelloAgentException>;

    fn stream_invoke(
        &self,
        messages: Vec<Message>,
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<
        Box<dyn Iterator<Item = Result<LlmResponse, HelloAgentException>> + Send>,
        HelloAgentException,
    >;

    async fn astream_invoke(
        &self,
        messages: Vec<Message>,
        kwargs: Option<HashMap<String, String>>,
    ) -> Pin<Box<dyn Stream<Item = Result<String, HelloAgentException>> + Send>>;

    fn invoke_with_tools(
        &self,
        messages: Vec<Message>,
        tools: Vec<HashMap<String, serde_json::Value>>,
        kwargs: HashMap<String, serde_json::Value>,
    ) -> Result<LlmToolResponse, HelloAgentException>;

    fn last_stats(&self) -> Option<StreamStats>;
}