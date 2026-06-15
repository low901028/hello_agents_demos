// infra/session/store.rs
use std::path::PathBuf;
use async_trait::async_trait;
use crate::core::traits::session_store::SessionStore;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::session::{SessionData, SessionInfo};

pub struct FileSessionStore {
    session_dir: PathBuf,
}

impl FileSessionStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { session_dir: dir.into() }
    }
}

#[async_trait]
impl SessionStore for FileSessionStore {
    async fn save(&self, session: &SessionData) -> Result<String, HelloAgentError> {
        let filename = format!("{}.json", session.session_id);
        let filepath = self.session_dir.join(&filename);
        let tmp = filepath.with_extension("tmp");
        let json = serde_json::to_vec_pretty(session)?;
        tokio::fs::write(&tmp, &json).await?;
        tokio::fs::rename(&tmp, &filepath).await?;
        Ok(filepath.to_string_lossy().into_owned())
    }

    async fn load(&self, path: &str) -> Result<SessionData, HelloAgentError> {
        let data = tokio::fs::read_to_string(path).await?;
        let session: SessionData = serde_json::from_str(&data)?;
        Ok(session)
    }

    async fn list_sessions(&self) -> Result<Vec<SessionInfo>, HelloAgentError> {
        let mut sessions = Vec::new();
        let mut dir = tokio::fs::read_dir(&self.session_dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(data) = tokio::fs::read_to_string(&path).await {
                    if let Ok(session) = serde_json::from_str::<SessionData>(&data) {
                        sessions.push(SessionInfo {
                            filename: entry.file_name().to_string_lossy().into_owned(),
                            session_id: session.session_id,
                            created_at: session.metadata.get("created_at")
                                .and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        });
                    }
                }
            }
        }
        Ok(sessions)
    }
}