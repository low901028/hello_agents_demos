use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    pub status: ToolStatus,
    pub text: String,
    #[serde(default)]
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_info: Option<ErrorInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl ToolResponse {
    // 简洁成功响应
    pub fn success(text: impl Into<String>) -> Self {
        Self {
            status: ToolStatus::Success,
            text: text.into(),
            data: Value::Object(Default::default()),
            error_info: None,
            stats: None,
            context: None,
        }
    }

    // 带附加信息的成功响应
    pub fn success_with(
        text: impl Into<String>,
        data: Option<Value>,
        stats: Option<Value>,
        context: Option<Value>,
    ) -> Self {
        Self {
            status: ToolStatus::Success,
            text: text.into(),
            data: data.unwrap_or(Value::Object(Default::default())),
            error_info: None,
            stats,
            context,
        }
    }

    // 简洁错误响应
    pub fn error(code: &str, message: &str) -> Self {
        Self {
            status: ToolStatus::Error,
            text: message.into(),
            data: Value::Object(Default::default()),
            error_info: Some(ErrorInfo {
                code: code.into(),
                message: message.into(),
            }),
            stats: None,
            context: None,
        }
    }

    // 带附加信息的错误响应
    pub fn error_with(
        code: &str,
        message: &str,
        stats: Option<Value>,
        context: Option<Value>,
    ) -> Self {
        Self {
            status: ToolStatus::Error,
            text: message.into(),
            data: Value::Object(Default::default()),
            error_info: Some(ErrorInfo {
                code: code.into(),
                message: message.into(),
            }),
            stats,
            context,
        }
    }

    // 简洁部分成功响应
    pub fn partial(text: impl Into<String>) -> Self {
        Self {
            status: ToolStatus::Partial,
            text: text.into(),
            data: Value::Object(Default::default()),
            error_info: None,
            stats: None,
            context: None,
        }
    }

    // 带附加信息的部分成功响应
    pub fn partial_with(
        text: impl Into<String>,
        data: Option<Value>,
        stats: Option<Value>,
        context: Option<Value>,
    ) -> Self {
        Self {
            status: ToolStatus::Partial,
            text: text.into(),
            data: data.unwrap_or(Value::Object(Default::default())),
            error_info: None,
            stats,
            context,
        }
    }
}
