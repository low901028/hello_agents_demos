use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
    Summary,
}

impl MessageRole {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "user" => Some(MessageRole::User),
            "assistant" => Some(MessageRole::Assistant),
            "system" => Some(MessageRole::System),
            "tool" => Some(MessageRole::Tool),
            "summary" => Some(MessageRole::Summary),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
            MessageRole::Summary => "summary",
        }
    }
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub content: String,
    pub role: MessageRole,
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    pub fn new(content: impl Into<String>, role: MessageRole) -> Self {
        Message {
            content: content.into(),
            role,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(content, MessageRole::User)
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(content, MessageRole::Assistant)
    }
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(content, MessageRole::System)
    }
    pub fn tool(content: impl Into<String>) -> Self {
        Self::new(content, MessageRole::Tool)
    }
    pub fn summary(content: impl Into<String>) -> Self {
        Self::new(content, MessageRole::Summary)
    }
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
    pub fn to_openai_dict(&self) -> HashMap<String, serde_json::Value> {
        let mut dict = HashMap::with_capacity(2);
        dict.insert("role".into(), serde_json::json!(self.role.as_str()));
        dict.insert("content".into(), serde_json::json!(&self.content));
        dict
    }
    pub fn to_text(&self) -> String {
        format!("[{}] {}", self.role, self.content)
    }
    pub fn content_length(&self) -> usize {
        self.content.len()
    }
    pub fn truncate_content(&self, max_len: usize) -> String {
        if self.content.len() <= max_len {
            self.content.clone()
        } else {
            format!("{}...", &self.content[..max_len])
        }
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.role, self.content)
    }
}
