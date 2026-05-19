use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 流式事件类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreamEventType {
    #[serde(rename = "agent_start")]
    AgentStart,
    #[serde(rename = "agent_finish")]
    AgentFinish,
    #[serde(rename = "step_start")]
    StepStart,
    #[serde(rename = "step_finish")]
    StepFinish,
    #[serde(rename = "tool_call_start")]
    ToolCallStart,
    #[serde(rename = "tool_call_finish")]
    ToolCallFinish,
    #[serde(rename = "llm_chunk")]
    LlmChunk,
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(rename = "error")]
    Error,
}

impl StreamEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AgentStart => "agent_start",
            Self::AgentFinish => "agent_finish",
            Self::StepStart => "step_start",
            Self::StepFinish => "step_finish",
            Self::ToolCallStart => "tool_call_start",
            Self::ToolCallFinish => "tool_call_finish",
            Self::LlmChunk => "llm_chunk",
            Self::Thinking => "thinking",
            Self::Error => "error",
        }
    }
}

/// 流式事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    #[serde(rename = "type")]
    pub event_type: StreamEventType,
    pub timestamp: f64,
    pub agent_name: String,
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
}

impl StreamEvent {
    pub fn new(event_type: StreamEventType, agent_name: impl Into<String>) -> Self {
        Self {
            event_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
            agent_name: agent_name.into(),
            data: HashMap::new(),
        }
    }

    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    /// 转换为 SSE 格式
    pub fn to_sse(&self) -> String {
        let event_dict = serde_json::json!({
            "type": self.event_type.as_str(),
            "timestamp": self.timestamp,
            "agent_name": self.agent_name,
            "data": self.data,
        });
        format!(
            "event: {}\ndata: {}\n\n",
            self.event_type.as_str(),
            serde_json::to_string(&event_dict).unwrap_or_default()
        )
    }

    /// 转换为字典
    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

/// 流式输出缓冲区
pub struct StreamBuffer {
    max_buffer_size: usize,
    events: Vec<StreamEvent>,
}

impl StreamBuffer {
    pub fn new(max_buffer_size: usize) -> Self {
        Self {
            max_buffer_size,
            events: Vec::new(),
        }
    }

    pub fn add(&mut self, event: StreamEvent) {
        self.events.push(event);
        if self.events.len() > self.max_buffer_size {
            self.events.remove(0);
        }
    }

    pub fn get_all(&self) -> Vec<StreamEvent> {
        self.events.clone()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn filter_by_type(&self, event_type: &StreamEventType) -> Vec<StreamEvent> {
        self.events
            .iter()
            .filter(|e| &e.event_type == event_type)
            .cloned()
            .collect()
    }
}