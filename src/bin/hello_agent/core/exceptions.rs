use std::fmt;

/// HelloAgents 统一错误类型
#[derive(Debug)]
pub enum HelloAgentsError {
    LLM(String),
    Agent(String),
    Config(String),
    Tool(String),
    Network(String),
    Parse(String),
    Io(std::io::Error),
}

impl fmt::Display for HelloAgentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LLM(msg) => write!(f, "LLM异常: {}", msg),
            Self::Agent(msg) => write!(f, "Agent异常: {}", msg),
            Self::Config(msg) => write!(f, "配置异常: {}", msg),
            Self::Tool(msg) => write!(f, "工具异常: {}", msg),
            Self::Network(msg) => write!(f, "网络异常: {}", msg),
            Self::Parse(msg) => write!(f, "解析异常: {}", msg),
            Self::Io(e) => write!(f, "IO异常: {}", e),
        }
    }
}

impl std::error::Error for HelloAgentsError {}

impl From<std::io::Error> for HelloAgentsError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// 类型别名，保持与 Python 一致的命名
pub type HelloAgentsException = HelloAgentsError;
pub type LLMException = HelloAgentsError;
pub type AgentException = HelloAgentsError;
pub type ConfigException = HelloAgentsError;
pub type ToolException = HelloAgentsError;