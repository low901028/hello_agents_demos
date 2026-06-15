use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
/// =================================
/// agent操作事件
/// =================================
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub timestamp: f64,
    pub agent_name: String,
    #[serde(default)]
    pub data: HashMap<String, Value>,
}

impl AgentEvent {
    pub fn new(event_type: EventType, agent_name: String, data: HashMap<String, Value>) -> Self {
        Self {
            event_type,
            timestamp: Utc::now().timestamp_millis() as f64 / 1000.0,
            agent_name,
            data,
        }
    }
}

/// 流式事件（供 arun_stream 返回）
#[derive(Debug, Clone)]
pub enum StreamEvent {
    AgentStart {
        name: String,
        input_text: String,
    },
    AgentFinish {
        name: String,
        result: String,
        total_steps: usize,
    },
    AgentError {
        name: String,
        error: String,
        error_type: String,
    },
    StepStart {
        name: String,
        step: usize,
        max_steps: usize,
        description: Option<String>,
    },
    StepFinish {
        name: String,
        step: usize,
        result: Option<String>,
    },
    LlmChunk {
        name: String,
        chunk: String,
        step: usize,
    },
    ToolCallStart {
        name: String,
        tool_name: String,
        tool_call_id: String,
        args: Value,
        step: usize,
    },
    ToolCallFinish {
        name: String,
        tool_name: String,
        tool_call_id: String,
        result: String,
        step: usize,
    },
    Thinking {
        name: String,
        chunk: String,
        phase: String,
        iteration: usize,
    },
    Reflection {
        name: String,
        feedback: String,
        iteration: usize,
    },
    PlanGenerated {
        name: String,
        plan: Vec<String>,
    },
}
