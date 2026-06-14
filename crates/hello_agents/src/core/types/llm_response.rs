use serde::{Deserialize, Serialize};
use crate::core::types::llm_resp_req::{ToolCall, Usage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<Usage>,
    pub latency_ms: i64,
    pub reasoning_content: Option<String>,
    pub backward_compatibility: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmToolResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub model: String,
    pub usage: Usage,
    pub latency_ms: i64,
}

impl LlmToolResponse {
    pub fn new(
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
        model: String,
        usage: Usage,
        latency_ms: i64,
    ) -> Self {
        Self {
            content,
            tool_calls,
            model,
            usage,
            latency_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStats {
    pub model: String,
    pub usage: Usage,
    pub latency_ms: i64,
    pub reasoning_content: Option<String>,
}