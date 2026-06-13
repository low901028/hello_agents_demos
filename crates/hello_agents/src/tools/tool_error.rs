//! tool_errors.rs
//! 工具错误码定义 - 标准化错误码，对应 Python ToolErrorCode

/// 资源相关错误
/// 工具错误码枚举
///
/// 标准化错误码，用于统一错误处理和追踪。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolErrorCode {
    /// 资源不存在（文件、工具等）
    NotFound,
    /// 访问被拒绝
    AccessDenied,
    /// 权限不足
    PermissionDenied,
    /// 期望文件但得到目录
    IsDirectory,
    /// 二进制文件无法处理
    BinaryFile,
    /// 参数无效或缺失
    InvalidParam,
    /// 格式错误
    InvalidFormat,
    /// 执行过程中发生错误
    ExecutionError,
    /// 执行超时
    Timeout,
    /// 内部错误
    InternalError,
    /// 冲突（如乐观锁冲突）
    Conflict,
    /// 熔断器开启，拒绝执行
    CircuitOpen,
    /// 网络请求失败
    NetworkError,
    /// API 调用失败
    ApiError,
    /// 速率限制
    RateLimit,
}

impl ToolErrorCode {
    /// 获取错误码的字符串表示
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

    /// 从字符串解析错误码
    pub fn from_str(code: &str) -> Option<Self> {
        match code {
            "NOT_FOUND" => Some(ToolErrorCode::NotFound),
            "ACCESS_DENIED" => Some(ToolErrorCode::AccessDenied),
            "PERMISSION_DENIED" => Some(ToolErrorCode::PermissionDenied),
            "IS_DIRECTORY" => Some(ToolErrorCode::IsDirectory),
            "BINARY_FILE" => Some(ToolErrorCode::BinaryFile),
            "INVALID_PARAM" => Some(ToolErrorCode::InvalidParam),
            "INVALID_FORMAT" => Some(ToolErrorCode::InvalidFormat),
            "EXECUTION_ERROR" => Some(ToolErrorCode::ExecutionError),
            "TIMEOUT" => Some(ToolErrorCode::Timeout),
            "INTERNAL_ERROR" => Some(ToolErrorCode::InternalError),
            "CONFLICT" => Some(ToolErrorCode::Conflict),
            "CIRCUIT_OPEN" => Some(ToolErrorCode::CircuitOpen),
            "NETWORK_ERROR" => Some(ToolErrorCode::NetworkError),
            "API_ERROR" => Some(ToolErrorCode::ApiError),
            "RATE_LIMIT" => Some(ToolErrorCode::RateLimit),
            _ => None,
        }
    }

    /// 获取所有错误码字符串
    pub fn get_all_codes() -> Vec<&'static str> {
        vec![
            "NOT_FOUND",
            "ACCESS_DENIED",
            "PERMISSION_DENIED",
            "IS_DIRECTORY",
            "BINARY_FILE",
            "INVALID_PARAM",
            "INVALID_FORMAT",
            "EXECUTION_ERROR",
            "TIMEOUT",
            "INTERNAL_ERROR",
            "CONFLICT",
            "CIRCUIT_OPEN",
            "NETWORK_ERROR",
            "API_ERROR",
            "RATE_LIMIT",
        ]
    }

    /// 检查给定的字符串是否为有效错误码
    pub fn is_valid_code(code: &str) -> bool {
        Self::from_str(code).is_some()
    }
}

impl std::fmt::Display for ToolErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(ToolErrorCode::NotFound.as_str(), "NOT_FOUND");
        assert_eq!(ToolErrorCode::CircuitOpen.as_str(), "CIRCUIT_OPEN");
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            ToolErrorCode::from_str("NOT_FOUND"),
            Some(ToolErrorCode::NotFound)
        );
        assert_eq!(ToolErrorCode::from_str("INVALID"), None);
    }

    #[test]
    fn test_get_all_codes() {
        let codes = ToolErrorCode::get_all_codes();
        assert!(codes.contains(&"NOT_FOUND"));
        assert!(codes.contains(&"RATE_LIMIT"));
        assert_eq!(codes.len(), 15);
    }

    #[test]
    fn test_is_valid_code() {
        assert!(ToolErrorCode::is_valid_code("TIMEOUT"));
        assert!(!ToolErrorCode::is_valid_code("UNKNOWN"));
    }
}