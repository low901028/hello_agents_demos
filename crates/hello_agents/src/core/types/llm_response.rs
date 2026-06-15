use crate::core::types::message::{ToolCall, Usage};

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Usage,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub delta_content: Option<String>,
    pub reasoning_content: Option<String>,
    pub finish_reason: Option<String>,
}
