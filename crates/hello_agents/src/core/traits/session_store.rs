use async_trait::async_trait;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::session::{SessionData, SessionInfo};

/// =================================
/// session记录
/// =================================
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn save(&self, session: &SessionData) -> Result<String, HelloAgentError>;
    async fn load(&self, path: &str) -> Result<SessionData, HelloAgentError>;
    async fn list_sessions(&self) -> Result<Vec<SessionInfo>, HelloAgentError>;
}
