use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 消息角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
    Summary,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
            Self::Summary => "summary",
        }
    }
}

impl From<&str> for MessageRole {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "user" => Self::User,
            "assistant" => Self::Assistant,
            "system" => Self::System,
            "tool" => Self::Tool,
            "summary" => Self::Summary,
            _ => Self::User,
        }
    }
}

/// 消息类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub content: String,
    pub role: MessageRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Message {
    pub fn new(content: impl Into<String>, role: impl Into<MessageRole>) -> Self {
        Self {
            content: content.into(),
            role: role.into(),
            timestamp: Some(Utc::now()),
            metadata: None,
        }
    }

    pub fn new_with_metadata(
        content: impl Into<String>,
        role: impl Into<MessageRole>,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            content: content.into(),
            role: role.into(),
            timestamp: Some(Utc::now()),
            metadata: Some(metadata),
        }
    }

    /// 转换为字典格式（OpenAI API 格式）
    pub fn to_dict(&self) -> serde_json::Value {
        let mut map = serde_json::json!({
            "role": self.role.as_str(),
            "content": self.content,
        });
        if let Some(ref ts) = self.timestamp {
            map["timestamp"] = serde_json::json!(ts.to_rfc3339());
        }
        if let Some(ref meta) = self.metadata {
            map["metadata"] = meta.clone();
        }
        map
    }

    /// 从字典创建消息
    pub fn from_dict(data: &serde_json::Value) -> Option<Self> {
        let content = data.get("content")?.as_str()?;
        let role = data.get("role")?.as_str()?;
        let timestamp = data
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        let metadata = data.get("metadata").cloned();

        Some(Self {
            content: content.to_string(),
            role: role.into(),
            timestamp,
            metadata,
        })
    }

    /// 格式化为文本
    pub fn to_text(&self) -> String {
        format!("[{}] {}", self.role.as_str(), self.content)
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.role.as_str(), self.content)
    }
}