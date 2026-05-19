use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 工具执行状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    Success,
    Partial,
    Error,
}

/// 工具响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub status: ToolStatus,
    pub text: String,
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
    pub error_info: Option<HashMap<String, String>>,
    pub stats: Option<HashMap<String, serde_json::Value>>,
    pub context: Option<HashMap<String, serde_json::Value>>,
}

impl ToolResponse {
    /// 创建成功响应
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            status: ToolStatus::Success,
            text: text.into(),
            data: HashMap::new(),
            error_info: None,
            stats: None,
            context: None,
        }
    }

    /// 创建部分成功响应
    pub fn partial(text: impl Into<String>) -> Self {
        Self {
            status: ToolStatus::Partial,
            text: text.into(),
            data: HashMap::new(),
            error_info: None,
            stats: None,
            context: None,
        }
    }

    /// 创建错误响应
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        let message = &message.into();
        let mut error_info = HashMap::new();
        error_info.insert("code".into(), code.into());
        error_info.insert("message".into(), message.into());

        Self {
            status: ToolStatus::Error,
            text: message.into(),
            data: HashMap::new(),
            error_info: Some(error_info),
            stats: None,
            context: None,
        }
    }

    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }

    pub fn with_stats(mut self, stats: HashMap<String, serde_json::Value>) -> Self {
        self.stats = Some(stats);
        self
    }

    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}