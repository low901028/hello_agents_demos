//! core/types/exceptions.rs
//! 异常类
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HelloAgentError {
    #[error("LLM call failed: {0}")]
    Llm(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("Agent error: {0}")]
    Agent(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("Tool error: {0}")]
    Tool(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Stream ended")]
    StreamEnd,

    #[error("{0}")]
    General(String),
}
