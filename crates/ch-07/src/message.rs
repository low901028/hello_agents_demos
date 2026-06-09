use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use simple_hello_agents_base::client_message::Message;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageRole {
    USER,
    ASSISTANT,
    SYSTEM,
    TOOL,
}

impl MessageRole {
    /// role 文字表示
    pub fn val_to_str(&self) -> &'static str {
        match self {
            MessageRole::USER => "user",
            MessageRole::ASSISTANT => "assistant",
            MessageRole::SYSTEM => "system",
            MessageRole::TOOL => "tool",
        }
    }

    /// role 文字转enum
    pub fn str_to_val(role: &str) -> MessageRole {
        match role {
            "user" => MessageRole::USER,
            "assistant" => MessageRole::ASSISTANT,
            "system" => MessageRole::SYSTEM,
            "tool" => MessageRole::TOOL,
            _ => { panic!("Unknown role: {}", role); }
        }
    }
}

/// ==================================
/// 构建新的message 满足metadata统计
/// ==================================
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageV2{
    message: Message,
    role: MessageRole,
    timestamp: Option<usize>,
    metadata: Option<HashMap<String, serde_json::Value>>,
}

impl MessageV2 {
    pub fn new(msg: Message, timestamp: Option<usize>, metadata: Option<HashMap<String, serde_json::Value>>) -> MessageV2 {
        MessageV2 {
            role: MessageRole::str_to_val(msg.role.as_str()),
            message: msg,
            metadata: metadata.or_else(||{ Some(HashMap::<String, serde_json::Value>::new()) }),
            timestamp: timestamp.or_else(|| { Some(chrono::Utc::now().timestamp() as usize)}),
        }
    }
}

/// ======================================
/// 请求体: 需要支持MessageV2
/// ======================================
#[derive(Debug, Serialize)]
pub struct ChatRequestV2 {
    pub model: String,
    pub messages: Vec<MessageV2>,
    pub temperature: f32,
    pub stream: bool,
}
