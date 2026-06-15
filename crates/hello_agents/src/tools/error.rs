//! ============================================================
//! src/tools/error.rs
//! ============================================================
/// 工具错误码枚举
#[derive(Debug, Clone, PartialEq)]
pub enum ToolErrorCode {
    NotFound,
    AccessDenied,
    PermissionDenied,
    IsDirectory,
    BinaryFile,
    InvalidParam,
    InvalidFormat,
    ExecutionError,
    Timeout,
    InternalError,
    Conflict,
    CircuitOpen,
    NetworkError,
    ApiError,
    RateLimit,
}

impl ToolErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotFound => "NOT_FOUND",
            Self::AccessDenied => "ACCESS_DENIED",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::IsDirectory => "IS_DIRECTORY",
            Self::BinaryFile => "BINARY_FILE",
            Self::InvalidParam => "INVALID_PARAM",
            Self::InvalidFormat => "INVALID_FORMAT",
            Self::ExecutionError => "EXECUTION_ERROR",
            Self::Timeout => "TIMEOUT",
            Self::InternalError => "INTERNAL_ERROR",
            Self::Conflict => "CONFLICT",
            Self::CircuitOpen => "CIRCUIT_OPEN",
            Self::NetworkError => "NETWORK_ERROR",
            Self::ApiError => "API_ERROR",
            Self::RateLimit => "RATE_LIMIT",
        }
    }
}
