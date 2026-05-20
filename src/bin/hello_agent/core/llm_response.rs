use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl ToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmToolResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub model: String,
    #[serde(default)]
    pub usage: HashMap<String, i64>,
    pub latency_ms: i64,
}

impl LlmToolResponse {
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
    pub fn total_tokens(&self) -> i64 {
        self.usage.get("total_tokens").copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    #[serde(default)]
    pub usage: HashMap<String, i64>,
    pub latency_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

impl LlmResponse {
    pub fn new(
        content: impl Into<String>,
        model: impl Into<String>,
        usage: HashMap<String, i64>,
        latency_ms: i64,
    ) -> Self {
        LlmResponse {
            content: content.into(),
            model: model.into(),
            usage,
            latency_ms,
            reasoning_content: None,
        }
    }
    pub fn with_reasoning(
        content: impl Into<String>,
        model: impl Into<String>,
        usage: HashMap<String, i64>,
        latency_ms: i64,
        r: impl Into<String>,
    ) -> Self {
        LlmResponse {
            content: content.into(),
            model: model.into(),
            usage,
            latency_ms,
            reasoning_content: Some(r.into()),
        }
    }
    pub fn total_tokens(&self) -> i64 {
        self.usage.get("total_tokens").copied().unwrap_or(0)
    }
    pub fn has_reasoning(&self) -> bool {
        self.reasoning_content.is_some()
    }
    pub fn content_length(&self) -> usize {
        self.content.len()
    }
}

impl std::fmt::Display for LlmResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStats {
    pub model: String,
    #[serde(default)]
    pub usage: HashMap<String, i64>,
    pub latency_ms: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

impl StreamStats {
    pub fn new(model: impl Into<String>, usage: HashMap<String, i64>, latency_ms: i64) -> Self {
        StreamStats {
            model: model.into(),
            usage,
            latency_ms,
            reasoning_content: None,
        }
    }
}
