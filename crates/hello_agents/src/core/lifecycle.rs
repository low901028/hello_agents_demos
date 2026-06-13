use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// ======================================================
/// Agent 异步生命周期事件系统
///
/// 提供事件驱动的 Agent 执行流程，支持：
/// - 生命周期钩子（on_start, on_step, on_finish, on_error）
/// - 流式事件输出（SSE/WebSocket 场景）
/// - 异步执行与并行优化
/// ======================================================
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
    pub fn val_as_str(&self) -> &'static str {
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
        write!(f, "{}", self.val_as_str())
    }
}


/// ===========================================
/// Agent 生命周期事件
///
///     所有事件的基础数据结构，包含：
///     - type: 事件类型
///     - timestamp: 时间戳
///     - agent_name: Agent 名称
///     - data: 事件数据（灵活扩展）
/// ===========================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub timestamp: f64,
    pub agent_name: String,
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
}

/// 创建事件的便捷方法
///
///         Args:
///             event_type: 事件类型
///             agent_name: Agent 名称
///             data: 事件数据（键值对）
///
///         Returns:
///             AgentEvent 实例
///
/// ```rust
/// let event = AgentEvent.new(
///      EventType.TOOL_CALL,
///      "my_agent",
///      tool_name="search",
///      tool_args={"query": "hello"}
/// ```
impl AgentEvent {
    pub fn new(
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

    /// 转换为字典（用于序列化）
    ///
    ///  Returns:
    ///      字典表示
    pub fn to_dict(&self) -> Option<HashMap<String, serde_json::Value>> {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::Object(map)) => Some(map.into_iter().collect()),
            _ => None,
        }
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

/// Agent 执行上下文
///
///     在异步执行过程中传递的上下文信息，包含：
///     - 输入文本
///     - 当前步骤
///     - 累计 token 数
///     - 自定义元数据
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

    /// 设置元数据
    pub fn set_metadata(mut self, key: &'static str, value: &'static str) {
        self.metadata.insert(key.to_string(), serde_json::json!(value));
    }

    /// 获取元数据
    pub fn get_metadata(self, key: &str) -> Option<serde_json::Value> {
        self.metadata.get(&key.to_string()).cloned()
    }
}

/// 生命周期钩子
pub type LifecycleHook =
Arc<dyn Fn(AgentEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;