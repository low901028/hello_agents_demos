//! ================================================
//! 流式输出支持 - SSE (Server-Sent Events) 实现
//! ================================================
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

/// ==========================
/// 流式事件类型
/// ==========================
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
    #[serde(rename = "llm_chunk")] // LLM流式输出
    LlmChunk,
    #[serde(rename = "thinking")] // agent思考过程
    Thinking,
    #[serde(rename = "error")]
    Error,
}

impl StreamEventType {
    pub fn val_as_str(&self) -> &'static str {
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
        write!(f, "{}", self.val_as_str())
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
    pub fn new(
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
    // pub fn with_data(
    //     event_type: StreamEventType,
    //     agent_name: impl Into<String>,
    //     key: impl Into<String>,
    //     value: impl Into<serde_json::Value>,
    // ) -> Self {
    //     let mut data = HashMap::with_capacity(1);
    //     data.insert(key.into(), value.into());
    //     Self::new(event_type, agent_name, data)
    // }

    /// 转换为 SSE 格式
    ///
    ///         SSE 格式：
    ///         event: <event_type>
    ///         data: <json_data>
    ///
    pub fn to_sse(&self) -> String {
        let json = serde_json::to_string(&self.to_dict()).unwrap_or_default();
        format!(
            "\nevent: {}\ndata: {}\n\n",
            self.event_type.val_as_str(),
            json
        ) //  【注】\n\n, 空行表示事件结束
    }

    /// 转换为字典
    pub fn to_dict(&self) -> HashMap<String, serde_json::Value> {
        let mut dict = HashMap::with_capacity(4);
        dict.insert(
            "type".into(),
            serde_json::json!(self.event_type.val_as_str()),
        );
        dict.insert("timestamp".into(), serde_json::json!(self.timestamp));
        dict.insert("agent_name".into(), serde_json::json!(&self.agent_name));
        dict.insert(
            "data".into(),
            serde_json::to_value(&self.data).unwrap_or_default(),
        );
        dict
    }
}

/// 流式输出缓冲区
///
///     用于收集和管理流式事件，支持：
///     - 事件缓冲
///     - 背压控制
///     - 事件过滤
pub struct StreamBuffer {
    max_buffer_size: usize,
    events: Vec<StreamEvent>,
}

impl StreamBuffer {
    pub fn new(max_buffer_size: Option<usize>) -> Self {
        // 默认：100
        let max_buffer_size = max_buffer_size.unwrap_or(100);
        StreamBuffer {
            max_buffer_size,
            events: Vec::with_capacity(max_buffer_size),
        }
    }
    /// 添加事件到缓冲区
    pub fn add(&mut self, event: StreamEvent) {
        self.events.push(event);

        // TODO
        // 简单的背压控制：超过最大缓冲区大小时丢弃旧事件
        // 这里不用pop(最新的元素会被pop，而我们需要淘汰的旧元素)，故而用remove(0) 淘汰最旧的元素
        // 同样背压的实现是非常的简单的
        if self.events.len() > self.max_buffer_size {
            self.events.remove(0);
        }
    }

    /// 获取所有的事件
    pub fn get_all(&self) -> &Vec<StreamEvent> {
        &self.events
    }

    /// 清空缓存区
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// ==================
    /// 按类型过滤事件
    /// ==================
    pub fn filter_by_type(&self, event_type: StreamEventType) -> Vec<StreamEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == event_type)
            .map(|e| e.clone())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

/// 将事件流转换为 SSE 格式
///
///     Args:
///         event_stream: 事件流
///         include_types: 包含的事件类型（None 表示全部）
///
///     Yields:
///         SSE 格式的字符串
///
pub fn stream_to_sse<S>(
    event_stream: S,
    include_types: Option<Vec<StreamEventType>>,
) -> impl Stream<Item = String>
where
    S: Stream<Item = StreamEvent> + Unpin,
{
    event_stream
        .filter(move |event| {
            let keep = if let Some(ref types) = include_types {
                types.contains(&event.event_type)
            } else {
                true
            };
            async move { keep }
        })
        .map(|event| event.to_sse())
}

/// 将事件流转换为 JSON Lines 格式
pub fn stream_to_json<S>(
    event_stream: S,
    include_types: Option<Vec<StreamEventType>>,
) -> impl Stream<Item = String>
where
    S: Stream<Item = StreamEvent> + Unpin,
{
    event_stream
        .filter(move |event| {
            let keep = if let Some(types) = &include_types {
                types.contains(&event.event_type)
            } else {
                true // None 表示包含所有类型
            };
            async move { keep }
        })
        .map(|event| {
            let dict = event.to_dict();
            // 序列化为 JSON 字符串，并添加换行符
            let json_str = serde_json::to_string(&dict).expect("JSON serialization failed");
            format!("{}\n", json_str)
        })
}

mod tests {
    use super::*;
    #[tokio::test]
    pub async fn test_stream_to_sse() {
        use futures::stream::iter;
        use futures::StreamExt; // 确保 .map, .filter 等方法可用

        // 模拟一个事件流
        let events = vec![
            StreamEvent::new(StreamEventType::AgentFinish, "hello".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::AgentStart, "".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::StepStart, "世界".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::LlmChunk, "chunk".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::Error, "error".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::Thinking, "思考".to_string(), HashMap::new()),
        ];
        let stream = iter(events);

        // 只保留 Message 类型的事件
        let sse_stream = stream_to_sse(stream, None/*Some(vec![StreamEventType::LlmChunk])*/);

        // 消费 SSE 流
        tokio::pin!(sse_stream);
        while let Some(sse) = sse_stream.next().await {
            println!("{}", sse);
        }
    }

    #[tokio::test]
    pub async fn test_stream_to_json() {
        use futures::stream::iter;

        // 创建测试事件流
        let events = vec![
            StreamEvent::new(StreamEventType::AgentFinish, "hello".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::AgentStart, "".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::StepStart, "世界".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::LlmChunk, "chunk".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::Error, "error".to_string(), HashMap::new()),
            StreamEvent::new(StreamEventType::Thinking, "思考".to_string(), HashMap::new()),
        ];
        let stream = iter(events);

        // 仅包含 Message 类型的事件
        let json_stream = stream_to_json(stream, None/*Some(vec![StreamEventType::AgentFinish])*/);

        tokio::pin!(json_stream);
        while let Some(line) = json_stream.next().await {
            print!("{}", line); // 输出每行 JSON
        }
    }
}
