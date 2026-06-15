//! ============================================================
//! src/infra/session_store_impl   (实现 SessionStore trait)
//! ============================================================
use crate::core::traits::session_store::SessionStore as SessionStoreTrait;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::session::{SessionData, SessionInfo};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct SessionStore {
    session_dir: PathBuf,
}

impl SessionStore {
    pub fn new(session_dir: &str) -> Result<Self, HelloAgentError> {
        let dir = PathBuf::from(session_dir);
        fs::create_dir_all(&dir)?;
        Ok(Self { session_dir: dir })
    }
}

impl SessionStoreTrait for SessionStore {
    fn save(&self, session: &SessionData) -> Result<String, HelloAgentError> {
        let filename = format!("{}.json", session.session_id);
        let filepath = self.session_dir.join(filename);
        let json = serde_json::to_string_pretty(session)?;
        let temp = filepath.with_extension("tmp");
        fs::write(&temp, &json)?;
        fs::rename(&temp, &filepath)?;
        Ok(filepath.to_string_lossy().to_string())
    }

    fn load(&self, path: &str) -> Result<SessionData, HelloAgentError> {
        let content = fs::read_to_string(path)?;
        let data = serde_json::from_str(&content)?;
        Ok(data)
    }

    fn list_sessions(&self) -> Result<Vec<SessionInfo>, HelloAgentError> {
        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.session_dir)? {
            let entry = entry?;
            if entry.path().extension().map_or(false, |e| e == "json") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                        sessions.push(SessionInfo {
                            filename: entry.file_name().to_string_lossy().into_owned(),
                            session_id: data.session_id,
                            created_at: data
                                .metadata
                                .get("created_at")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        });
                    }
                }
            }
        }
        Ok(sessions)
    }
}
