/// 工具错误码
pub struct ToolErrorCode;

impl ToolErrorCode {
    pub const NOT_FOUND: &'static str = "NOT_FOUND";
    pub const ACCESS_DENIED: &'static str = "ACCESS_DENIED";
    pub const PERMISSION_DENIED: &'static str = "PERMISSION_DENIED";
    pub const IS_DIRECTORY: &'static str = "IS_DIRECTORY";
    pub const BINARY_FILE: &'static str = "BINARY_FILE";
    pub const INVALID_PARAM: &'static str = "INVALID_PARAM";
    pub const INVALID_FORMAT: &'static str = "INVALID_FORMAT";
    pub const EXECUTION_ERROR: &'static str = "EXECUTION_ERROR";
    pub const TIMEOUT: &'static str = "TIMEOUT";
    pub const INTERNAL_ERROR: &'static str = "INTERNAL_ERROR";
    pub const CONFLICT: &'static str = "CONFLICT";
    pub const CIRCUIT_OPEN: &'static str = "CIRCUIT_OPEN";
    pub const NETWORK_ERROR: &'static str = "NETWORK_ERROR";
    pub const API_ERROR: &'static str = "API_ERROR";
    pub const RATE_LIMIT: &'static str = "RATE_LIMIT";

    pub fn all_codes() -> Vec<&'static str> {
        vec![
            Self::NOT_FOUND, Self::ACCESS_DENIED, Self::PERMISSION_DENIED,
            Self::IS_DIRECTORY, Self::BINARY_FILE, Self::INVALID_PARAM,
            Self::INVALID_FORMAT, Self::EXECUTION_ERROR, Self::TIMEOUT,
            Self::INTERNAL_ERROR, Self::CONFLICT, Self::CIRCUIT_OPEN,
            Self::NETWORK_ERROR, Self::API_ERROR, Self::RATE_LIMIT,
        ]
    }
}