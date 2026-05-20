use futures::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
            StreamEventType::AgentStart => "agent_start",
            StreamEventType::AgentFinish => "agent_finish",
            StreamEventType::StepStart => "step_start",
            StreamEventType::StepFinish => "step_finish",
            StreamEventType::ToolCallStart => "tool_call_start",
            StreamEventType::ToolCallFinish => "tool_call_finish",
            StreamEventType::LlmChunk => "llm_chunk",
            StreamEventType::Thinking => "thinking",
            StreamEventType::Error => "error",
        }
    }
}

impl fmt::Display for StreamEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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
    pub fn create(
        event_type: StreamEventType,
        agent_name: impl Into<String>,
        data: HashMap<String, serde_json::Value>,
    ) -> Self {
        StreamEvent {
            event_type,
            timestamp: chrono::Utc::now().timestamp_millis() as f64 / 1000.0,
            agent_name: agent_name.into(),
            data,
        }
    }
    pub fn with_data(
        event_type: StreamEventType,
        agent_name: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        let mut data = HashMap::with_capacity(1);
        data.insert(key.into(), value.into());
        Self::create(event_type, agent_name, data)
    }
    pub fn to_sse(&self) -> String {
        let json = serde_json::to_string(&self.to_dict()).unwrap_or_default();
        format!("event: {}\ndata: {}\n\n", self.event_type.as_str(), json)
    }
    pub fn to_dict(&self) -> HashMap<String, serde_json::Value> {
        let mut dict = HashMap::with_capacity(4);
        dict.insert("type".into(), serde_json::json!(self.event_type.as_str()));
        dict.insert("timestamp".into(), serde_json::json!(self.timestamp));
        dict.insert("agent_name".into(), serde_json::json!(&self.agent_name));
        dict.insert(
            "data".into(),
            serde_json::to_value(&self.data).unwrap_or_default(),
        );
        dict
    }
}

pub struct StreamBuffer {
    max_size: usize,
    events: Vec<StreamEvent>,
}

impl StreamBuffer {
    pub fn new(max_size: usize) -> Self {
        StreamBuffer {
            max_size,
            events: Vec::with_capacity(max_size),
        }
    }
    pub fn add(&mut self, event: StreamEvent) {
        self.events.push(event);
        if self.events.len() > self.max_size {
            self.events.remove(0);
        }
    }
    pub fn clear(&mut self) {
        self.events.clear();
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
}

pub struct SseStream {
    rx: mpsc::Receiver<StreamEvent>,
    include_types: Option<Vec<StreamEventType>>,
}

impl Stream for SseStream {
    type Item = String;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.rx.poll_recv(cx) {
                Poll::Ready(Some(event)) => {
                    if let Some(ref types) = self.include_types {
                        if !types.contains(&event.event_type) {
                            continue;
                        }
                    }
                    return Poll::Ready(Some(event.to_sse()));
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

pub fn create_event_channel(
    size: usize,
) -> (mpsc::Sender<StreamEvent>, mpsc::Receiver<StreamEvent>) {
    mpsc::channel(size)
}
