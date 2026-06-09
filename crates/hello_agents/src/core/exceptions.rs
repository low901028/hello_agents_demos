use thiserror::Error;

/// ===========================
/// 异常定义
/// ===========================
#[derive(Error, Debug)]
pub enum HelloAgentException {
    #[error("LLM错误: {0}")]
    LlmException(String),
    #[error("Agent错误: {0}")]
    AgentException(String),
    #[error("配置错误: {0}")]
    ConfigException(String),
    #[error("工具错误: {0}")]
    ToolException(String),
    #[error("网络错误: {0}")]
    NetworkException(String),
    #[error("IO错误: {0}")]
    IoException(#[from] std::io::Error),
    #[error("序列化错误: {0}")]
    SerializationException(String),
    #[error("{0}")]
    General(String),
}

/// ===============================
/// 异常转换
/// ===============================
impl HelloAgentException {
    #[inline]
    pub fn llm(msg: impl Into<String>) -> Self {
        HelloAgentException::LlmException(msg.into())
    }
    #[inline]
    pub fn agent(msg: impl Into<String>) -> Self {
        HelloAgentException::AgentException(msg.into())
    }
    #[inline]
    pub fn config(msg: impl Into<String>) -> Self {
        HelloAgentException::ConfigException(msg.into())
    }
    #[inline]
    pub fn tool(msg: impl Into<String>) -> Self {
        HelloAgentException::ToolException(msg.into())
    }
    pub fn is_llm_error(&self) -> bool {
        matches!(self, HelloAgentException::LlmException(_))
    }
}

impl From<serde_json::Error> for HelloAgentException {
    fn from(err: serde_json::Error) -> Self {
        HelloAgentException::SerializationException(err.to_string())
    }
}
impl From<serde_yaml::Error> for HelloAgentException {
    fn from(err: serde_yaml::Error) -> Self {
        HelloAgentException::SerializationException(err.to_string())
    }
}
impl From<reqwest::Error> for HelloAgentException {
    fn from(err: reqwest::Error) -> Self {
        HelloAgentException::NetworkException(err.to_string())
    }
}