use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Agent 生命周期事件类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    #[serde(rename = "agent_start")]
    AgentStart,
    #[serde(rename = "agent_finish")]
    AgentFinish,
    #[serde(rename = "agent_error")]
    AgentError,
    #[serde(rename = "step_start")]
    StepStart,
    #[serde(rename = "step_finish")]
    StepFinish,
    #[serde(rename = "llm_start")]
    LlmStart,
    #[serde(rename = "llm_chunk")]
    LlmChunk,
    #[serde(rename = "llm_finish")]
    LlmFinish,
    #[serde(rename = "tool_call")]
    ToolCall,
    #[serde(rename = "tool_result")]
    ToolResult,
    #[serde(rename = "tool_error")]
    ToolError,
    #[serde(rename = "thinking")]
    Thinking,
    #[serde(rename = "reflection")]
    Reflection,
    #[serde(rename = "plan")]
    Plan,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AgentStart => "agent_start",
            Self::AgentFinish => "agent_finish",
            Self::AgentError => "agent_error",
            Self::StepStart => "step_start",
            Self::StepFinish => "step_finish",
            Self::LlmStart => "llm_start",
            Self::LlmChunk => "llm_chunk",
            Self::LlmFinish => "llm_finish",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::ToolError => "tool_error",
            Self::Thinking => "thinking",
            Self::Reflection => "reflection",
            Self::Plan => "plan",
        }
    }
}

/// Agent 生命周期事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub timestamp: f64,
    pub agent_name: String,
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
}

impl AgentEvent {
    pub fn new(event_type: EventType, agent_name: impl Into<String>) -> Self {
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

    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

impl fmt::Display for AgentEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} @ {:.2}: {:?}",
            self.event_type.as_str(),
            self.agent_name,
            self.timestamp,
            self.data
        )
    }
}

/// 生命周期钩子类型
pub type LifecycleHook = Arc<dyn Fn(AgentEvent) -> futures::future::BoxFuture<'static, ()> + Send + Sync>;

/// 执行上下文
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub input_text: String,
    pub current_step: usize,
    pub total_tokens: usize,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExecutionContext {
    pub fn new(input_text: impl Into<String>) -> Self {
        Self {
            input_text: input_text.into(),
            current_step: 0,
            total_tokens: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn increment_step(&mut self) {
        self.current_step += 1;
    }

    pub fn add_tokens(&mut self, tokens: usize) {
        self.total_tokens += tokens;
    }

    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }
}