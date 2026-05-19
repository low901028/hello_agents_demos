use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 工具调用对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// 工具调用响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMToolResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub model: String,
    #[serde(default)]
    pub usage: HashMap<String, u32>,
    #[serde(default)]
    pub latency_ms: u64,
}

/// LLM 响应对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub content: String,
    pub model: String,
    #[serde(default)]
    pub usage: HashMap<String, u32>,
    #[serde(default)]
    pub latency_ms: u64,
    pub reasoning_content: Option<String>,
}

impl LLMResponse {
    pub fn new(
        content: String,
        model: String,
        usage: HashMap<String, u32>,
        latency_ms: u64,
        reasoning_content: Option<String>,
    ) -> Self {
        Self {
            content,
            model,
            usage,
            latency_ms,
            reasoning_content,
        }
    }

    pub fn to_dict(&self) -> serde_json::Value {
        let mut map = serde_json::json!({
            "content": self.content,
            "model": self.model,
            "usage": self.usage,
            "latency_ms": self.latency_ms,
        });
        if let Some(ref rc) = self.reasoning_content {
            map["reasoning_content"] = serde_json::json!(rc);
        }
        map
    }
}

impl std::fmt::Display for LLMResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.content)
    }
}

/// 流式统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamStats {
    pub model: String,
    #[serde(default)]
    pub usage: HashMap<String, u32>,
    #[serde(default)]
    pub latency_ms: u64,
    pub reasoning_content: Option<String>,
}