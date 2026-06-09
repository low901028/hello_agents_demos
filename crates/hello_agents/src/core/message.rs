use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

///  ======================================================
///！ agent与LLM间交互的消息格式
///  ======================================================

/// 消息的角色
/// - user
/// - assistant
/// - system
/// - tool
/// - summary
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    USER,
    ASSISTANT,
    SYSTEM,
    TOOL,
    SUMMARY,
}

impl MessageRole {
    fn val_to_str(&self) -> &'static str {
        match self {
            MessageRole::USER => "user",
            MessageRole::ASSISTANT => "assistant",
            MessageRole::SYSTEM => "system",
            MessageRole::TOOL => "tool",
            MessageRole::SUMMARY => "summary",
        }
    }

    fn str_to_val(role: &str) -> Option<MessageRole> {
        match role {
            "user" => Some(MessageRole::USER),
            "assistant" => Some(MessageRole::ASSISTANT),
            "system" => Some(MessageRole::SYSTEM),
            "tool" => Some(MessageRole::TOOL),
            "summary" => Some(MessageRole::SUMMARY),
            _ => None,
        }
    }
}
impl Display for MessageRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

/// ========================================
/// 消息类
/// ========================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub content: String,
    pub role: MessageRole,
    #[serde(default = "Utc::now")]
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl Message {
    pub fn new(content: String,
               role: MessageRole,
               metadata: Option<HashMap<String, serde_json::Value>>) -> Self {
        Message {
            content,
            role,
            timestamp: Utc::now(),
            metadata: metadata.or_else(|| {Some(HashMap::<String, serde_json::Value>::new())}),
        }
    }

    /// 转换为字典格式（OpenAI API格式）
    pub(crate) fn to_dict(self) -> HashMap<String, serde_json::Value> {
        let mut dict = HashMap::with_capacity(4);
        dict.insert("role".into(), serde_json::json!(&self.role.val_to_str()));
        dict.insert("content".into(), serde_json::json!(&self.content));
        (dict).insert("timestamp".into(), serde_json::json!(&self.timestamp.to_rfc3339()));
        (dict).insert("metadata".into(), serde_json::json!(&self.metadata));
        dict
    }

    /// map 转 Message
    pub fn from_dict(mut dict: HashMap<String, serde_json::Value>) -> Self {
        let timestamp = dict.remove("timestamp").unwrap();
        let role = dict.remove("role").unwrap();
        let content = dict.remove("content").unwrap();
        let metadata = dict.remove("metadata").unwrap();

        Message {
            role: MessageRole::str_to_val(serde_json::to_string(&role).unwrap().as_str()).unwrap(),
            content: content.as_str().unwrap().to_string(),
            timestamp: DateTime::parse_from_rfc3339(timestamp.as_str().unwrap())
                .unwrap()
                .to_utc(),
            metadata: serde_json::from_value(metadata).unwrap_or(None),
        }
    }

    /// 格式化为文本（用于上下文构建）
    pub fn to_text(&self) -> String {
        format!("[{}] {}", self.role, self.content)
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?} {:?}]",self.role, self.content)
    }
}