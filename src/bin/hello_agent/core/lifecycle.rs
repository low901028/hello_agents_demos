use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

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
            EventType::AgentStart => "agent_start",
            EventType::AgentFinish => "agent_finish",
            EventType::AgentError => "agent_error",
            EventType::StepStart => "step_start",
            EventType::StepFinish => "step_finish",
            EventType::LlmStart => "llm_start",
            EventType::LlmChunk => "llm_chunk",
            EventType::LlmFinish => "llm_finish",
            EventType::ToolCall => "tool_call",
            EventType::ToolResult => "tool_result",
            EventType::ToolError => "tool_error",
            EventType::Thinking => "thinking",
            EventType::Reflection => "reflection",
            EventType::Plan => "plan",
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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
    pub fn create(
        event_type: EventType,
        agent_name: impl Into<String>,
        data: HashMap<String, serde_json::Value>,
    ) -> Self {
        AgentEvent {
            event_type,
            timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
            agent_name: agent_name.into(),
            data,
        }
    }
    pub fn simple(event_type: EventType, agent_name: impl Into<String>) -> Self {
        Self::create(event_type, agent_name, HashMap::new())
    }
}

impl fmt::Display for AgentEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} @ {:.2}",
            self.event_type, self.agent_name, self.timestamp
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub input_text: String,
    pub current_step: usize,
    pub total_tokens: usize,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ExecutionContext {
    pub fn new(input_text: impl Into<String>) -> Self {
        ExecutionContext {
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
}

pub type LifecycleHook =
    Arc<dyn Fn(AgentEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;
