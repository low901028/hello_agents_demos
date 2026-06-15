use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::session::{SessionData, SessionInfo};

/// =================================
/// session记录
/// =================================
pub trait SessionStore: Send + Sync {
    fn save(&self, session: &SessionData) -> Result<String, HelloAgentError>;
    fn load(&self, path: &str) -> Result<SessionData, HelloAgentError>;
    fn list_sessions(&self) -> Result<Vec<SessionInfo>, HelloAgentError>;
}
