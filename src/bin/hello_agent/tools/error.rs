use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolErrorCode {
    #[serde(rename = "NOT_FOUND")]
    NotFound,
    #[serde(rename = "ACCESS_DENIED")]
    AccessDenied,
    #[serde(rename = "PERMISSION_DENIED")]
    PermissionDenied,
    #[serde(rename = "IS_DIRECTORY")]
    IsDirectory,
    #[serde(rename = "BINARY_FILE")]
    BinaryFile,
    #[serde(rename = "INVALID_PARAM")]
    InvalidParam,
    #[serde(rename = "INVALID_FORMAT")]
    InvalidFormat,
    #[serde(rename = "EXECUTION_ERROR")]
    ExecutionError,
    #[serde(rename = "TIMEOUT")]
    Timeout,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
    #[serde(rename = "CONFLICT")]
    Conflict,
    #[serde(rename = "CIRCUIT_OPEN")]
    CircuitOpen,
    #[serde(rename = "NETWORK_ERROR")]
    NetworkError,
    #[serde(rename = "API_ERROR")]
    ApiError,
    #[serde(rename = "RATE_LIMIT")]
    RateLimit,
}

impl ToolErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ToolErrorCode::NotFound => "NOT_FOUND",
            ToolErrorCode::AccessDenied => "ACCESS_DENIED",
            ToolErrorCode::PermissionDenied => "PERMISSION_DENIED",
            ToolErrorCode::IsDirectory => "IS_DIRECTORY",
            ToolErrorCode::BinaryFile => "BINARY_FILE",
            ToolErrorCode::InvalidParam => "INVALID_PARAM",
            ToolErrorCode::InvalidFormat => "INVALID_FORMAT",
            ToolErrorCode::ExecutionError => "EXECUTION_ERROR",
            ToolErrorCode::Timeout => "TIMEOUT",
            ToolErrorCode::InternalError => "INTERNAL_ERROR",
            ToolErrorCode::Conflict => "CONFLICT",
            ToolErrorCode::CircuitOpen => "CIRCUIT_OPEN",
            ToolErrorCode::NetworkError => "NETWORK_ERROR",
            ToolErrorCode::ApiError => "API_ERROR",
            ToolErrorCode::RateLimit => "RATE_LIMIT",
        }
    }
}

impl std::fmt::Display for ToolErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
