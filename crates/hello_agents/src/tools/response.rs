//! tool_response.rs
//!工具响应协议
//!
//! 标准化的工具响应格式，提供结构化的状态、数据和错误信息。
// src/tools/response.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub data: Value,
    #[serde(skip_serializing_if = "Option::is_none", rename = "error")]
    pub error_info: Option<ErrorInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

impl ToolResponse {
    pub fn success(
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

    pub fn partial(
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

    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        stats: Option<Value>,
        context: Option<Value>,
    ) -> Self {
        let message = message.into().to_owned();
        Self {
            status: ToolStatus::Error,
            text: message.clone(),
            data: Value::Object(Default::default()),
            error_info: Some(ErrorInfo {
                code: code.into(),
                message: message.clone(),
            }),
            stats,
            context,
        }
    }

    pub fn to_dict(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_dict(value: &Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value.clone())
    }

    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

// ---------- 测试 ----------
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_success_response() {
        let resp = ToolResponse::success(
            "计算结果: 42",
            Some(json!({"result": 42, "expression": "6*7"})),
            Some(json!({"time_ms": 5})),
            Some(json!({"env": "test"})),
        );
        assert_eq!(resp.status, ToolStatus::Success);
        assert_eq!(resp.text, "计算结果: 42");
        assert_eq!(resp.data, json!({"result": 42, "expression": "6*7"}));
        assert!(resp.error_info.is_none());
        assert_eq!(resp.stats, Some(json!({"time_ms": 5})));
        assert_eq!(resp.context, Some(json!({"env": "test"})));
    }

    #[test]
    fn test_error_response() {
        let resp = ToolResponse::error("INVALID_PARAM", "表达式不能为空", None, None);
        assert_eq!(resp.status, ToolStatus::Error);
        assert_eq!(resp.text, "表达式不能为空");
        assert!(resp.data.as_object().unwrap().is_empty());
        let err = resp.error_info.unwrap();
        assert_eq!(err.code, "INVALID_PARAM");
        assert_eq!(err.message, "表达式不能为空");
    }

    #[test]
    fn test_partial_response() {
        let resp = ToolResponse::partial(
            "部分结果（输出被截断）",
            Some(json!({"result": "partial"})),
            None,
            None,
        );
        assert_eq!(resp.status, ToolStatus::Partial);
        assert_eq!(resp.text, "部分结果（输出被截断）");
    }

    #[test]
    fn test_to_dict_and_from_dict() {
        let original = ToolResponse::success("hello", Some(json!({"key": "value"})), None, None);
        let dict = original.to_dict();
        // 检查字段
        assert_eq!(dict["status"], "success");
        assert_eq!(dict["text"], "hello");
        assert_eq!(dict["test_data"], json!({"key": "value"}));
        assert!(!dict.as_object().unwrap().contains_key("error"));

        let restored = ToolResponse::from_dict(&dict).unwrap();
        assert_eq!(restored.status, ToolStatus::Success);
        assert_eq!(restored.text, "hello");
        assert_eq!(restored.data, json!({"key": "value"}));
        assert!(restored.error_info.is_none());
    }

    #[test]
    fn test_error_info_serialization() {
        let resp = ToolResponse::error("E001", "something went wrong", None, None);
        let json_val = resp.to_dict();
        assert_eq!(json_val["error"]["code"], "E001");
        assert_eq!(json_val["error"]["message"], "something went wrong");

        let restored = ToolResponse::from_dict(&json_val).unwrap();
        let err = restored.error_info.unwrap();
        assert_eq!(err.code, "E001");
        assert_eq!(err.message, "something went wrong");
    }

    #[test]
    fn test_to_json_and_from_json() {
        let original = ToolResponse::success(
            "ok",
            Some(json!({"foo": 1})),
            Some(json!({"time": 0.5})),
            Some(json!({"session": "xyz"})),
        );
        let json_str = original.to_json();
        let parsed = ToolResponse::from_json(&json_str).unwrap();
        assert_eq!(parsed.status, original.status);
        assert_eq!(parsed.text, original.text);
        assert_eq!(parsed.data, original.data);
        assert_eq!(parsed.stats, original.stats);
        assert_eq!(parsed.context, original.context);
    }

    #[test]
    fn test_from_dict_missing_optional_fields() {
        let dict = json!({
            "status": "success",
            "text": "minimal",
            "test_data": {}
        });
        let resp = ToolResponse::from_dict(&dict).unwrap();
        assert!(resp.error_info.is_none());
        assert!(resp.stats.is_none());
        assert!(resp.context.is_none());
    }
}
