use thiserror::Error;

#[derive(Error, Debug)]
pub enum HelloAgentsError {
    #[error("LLM错误: {0}")]
    LlmError(String),
    #[error("Agent错误: {0}")]
    AgentError(String),
    #[error("配置错误: {0}")]
    ConfigError(String),
    #[error("工具错误: {0}")]
    ToolError(String),
    #[error("网络错误: {0}")]
    NetworkError(String),
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),
    #[error("序列化错误: {0}")]
    SerializationError(String),
    #[error("{0}")]
    General(String),
}

pub type LlmError = HelloAgentsError;
pub type AgentError = HelloAgentsError;
pub type ConfigError = HelloAgentsError;
pub type ToolError = HelloAgentsError;

impl HelloAgentsError {
    #[inline]
    pub fn llm(msg: impl Into<String>) -> Self {
        HelloAgentsError::LlmError(msg.into())
    }
    #[inline]
    pub fn agent(msg: impl Into<String>) -> Self {
        HelloAgentsError::AgentError(msg.into())
    }
    #[inline]
    pub fn config(msg: impl Into<String>) -> Self {
        HelloAgentsError::ConfigError(msg.into())
    }
    #[inline]
    pub fn tool(msg: impl Into<String>) -> Self {
        HelloAgentsError::ToolError(msg.into())
    }
    pub fn is_llm_error(&self) -> bool {
        matches!(self, HelloAgentsError::LlmError(_))
    }
}

impl From<serde_json::Error> for HelloAgentsError {
    fn from(err: serde_json::Error) -> Self {
        HelloAgentsError::SerializationError(err.to_string())
    }
}
impl From<serde_yaml::Error> for HelloAgentsError {
    fn from(err: serde_yaml::Error) -> Self {
        HelloAgentsError::SerializationError(err.to_string())
    }
}
impl From<reqwest::Error> for HelloAgentsError {
    fn from(err: reqwest::Error) -> Self {
        HelloAgentsError::NetworkError(err.to_string())
    }
}

pub type HelloAgentsResult<T> = Result<T, HelloAgentsError>;
