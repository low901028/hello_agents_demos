use serde::{Deserialize, Serialize};
/// 一条消息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// 请求体
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub stream: bool,
}

/// 流式响应中的单个块
#[derive(Debug, Deserialize)]
pub struct ChatChunk {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub delta: Delta,
}

#[derive(Debug, Deserialize)]
pub struct Delta {
    pub content: Option<String>,
}

/// 非流式响应（一次性返回）
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<FinalChoice>,
}

#[derive(Debug, Deserialize)]
pub struct FinalChoice {
    pub message: Message,
}