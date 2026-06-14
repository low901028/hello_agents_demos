use thiserror::Error;

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
    #[error("流结束标记")]
    StreamEnd,
    #[error("{0}")]
    General(String),
}

impl HelloAgentException {
    pub fn llm(msg: impl Into<String>) -> Self {
        Self::LlmException(msg.into())
    }
    pub fn agent(msg: impl Into<String>) -> Self {
        Self::AgentException(msg.into())
    }
    pub fn config(msg: impl Into<String>) -> Self {
        Self::ConfigException(msg.into())
    }
    pub fn tool(msg: impl Into<String>) -> Self {
        Self::ToolException(msg.into())
    }
}

impl From<serde_json::Error> for HelloAgentException {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationException(e.to_string())
    }
}

impl From<reqwest::Error> for HelloAgentException {
    fn from(e: reqwest::Error) -> Self {
        Self::NetworkException(e.to_string())
    }
}