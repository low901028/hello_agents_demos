use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env::consts::ARCH;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    Success,
    Partial,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub status: ToolStatus,
    pub text: String,
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_info: Option<ErrorInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<HashMap<String, serde_json::Value>>,
}

impl ToolResponse {
    pub fn success(text: impl Into<String>, data: HashMap<String, serde_json::Value>) -> Self {
        ToolResponse {
            status: ToolStatus::Success,
            text: text.into(),
            data,
            error_info: None,
            stats: None,
            context: None,
        }
    }
    pub fn partial(text: impl Into<String>, data: HashMap<String, serde_json::Value>) -> Self {
        ToolResponse {
            status: ToolStatus::Partial,
            text: text.into(),
            data,
            error_info: None,
            stats: None,
            context: None,
        }
    }
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        let message = (message.into());
        ToolResponse {
            status: ToolStatus::Error,
            text: message.to_owned(),
            data: HashMap::new(),
            error_info: Some(ErrorInfo {
                code: code.into(),
                message: message.to_owned(),
            }),
            stats: None,
            context: None,
        }
    }
    pub fn with_stats(mut self, stats: HashMap<String, serde_json::Value>) -> Self {
        self.stats = Some(stats);
        self
    }
    pub fn with_context(mut self, context: HashMap<String, serde_json::Value>) -> Self {
        self.context = Some(context);
        self
    }
    pub fn with_data(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }
    pub fn is_success(&self) -> bool {
        self.status == ToolStatus::Success
    }
    pub fn is_error(&self) -> bool {
        self.status == ToolStatus::Error
    }
    pub fn is_partial(&self) -> bool {
        self.status == ToolStatus::Partial
    }
}
