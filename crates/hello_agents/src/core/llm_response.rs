use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::llm_resp_req::{ToolCall, Usage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<Usage>,
    pub latency_ms: i64,
    pub reasoning_content: Option<String>,
    pub backward_compatibility: bool,
}

impl LlmResponse {
    pub fn new(
        content: impl Into<String>,
        model: impl Into<String>,
        usage: Option<Usage>,
        latency_ms: i64,
        backward_compatibility: Option<bool>,
    ) -> Self {
        Self {
            content: content.into(),
            model: model.into(),
            usage,
            latency_ms,
            reasoning_content: None,
            backward_compatibility: backward_compatibility.unwrap_or(true),
        }
    }

    pub fn to_dict(&self) -> HashMap<String, serde_json::Value> {
        let mut dict = HashMap::new();
        dict.insert("content".into(), self.content.clone().into());
        dict.insert("model".into(), self.model.clone().into());
        dict.insert("usage".into(), serde_json::to_value(&self.usage).unwrap_or_default());
        dict.insert("latency_ms".into(), self.latency_ms.into());
        if let Some(ref r) = self.reasoning_content {
            dict.insert("reasoning_content".into(), r.clone().into());
        }
        dict
    }
}

impl std::fmt::Display for LlmResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.backward_compatibility {
            write!(f, "{}", self.content)
        } else {
            let total = self.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0);
            write!(f, "LLMResponse(model={}, latency={}ms, tokens={}", self.model, self.latency_ms, total)?;
            if self.reasoning_content.is_some() {
                write!(f, ", has_reasoning=True")?;
            }
            write!(f, ", content_length={})", self.content.len())
        }
    }
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
        Self { content, tool_calls, model, usage, latency_ms }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStats {
    pub model: String,
    pub usage: Usage,
    pub latency_ms: i64,
    pub reasoning_content: Option<String>,
}

impl StreamStats {
    pub fn new(model: String, usage: Usage, latency_ms: i64) -> Self {
        Self { model, usage, latency_ms, reasoning_content: None }
    }
}