use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{self};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SessionStore {
    session_dir: PathBuf,
}

impl SessionStore {
    pub fn new(session_dir: &str) -> io::Result<Self> {
        let dir = PathBuf::from(session_dir);
        fs::create_dir_all(&dir)?;
        Ok(SessionStore { session_dir: dir })
    }

    fn generate_session_id(&self) -> String {
        format!(
            "s-{}-{}",
            Utc::now().format("%Y%m%d-%H%M%S"),
            &Uuid::new_v4().to_string()[..8]
        )
    }

    pub fn save(
        &self,
        agent_config: &HashMap<String, serde_json::Value>,
        history: &[crate::hello_agent::core::message::Message],
        tool_schema_hash: &str,
        read_cache: &HashMap<String, HashMap<String, serde_json::Value>>,
        metadata: &HashMap<String, serde_json::Value>,
        session_name: Option<&str>,
    ) -> io::Result<String> {
        let sid = self.generate_session_id();
        let filename = session_name
            .map(|n| format!("{}.json", n))
            .unwrap_or_else(|| format!("session-{}.json", sid));
        let filepath = self.session_dir.join(&filename);
        let mut data = serde_json::Map::new();
        data.insert("session_id".into(), serde_json::json!(&sid));
        data.insert(
            "created_at".into(),
            serde_json::json!(
                metadata
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
            ),
        );
        data.insert(
            "saved_at".into(),
            serde_json::json!(Utc::now().to_rfc3339()),
        );
        data.insert(
            "agent_config".into(),
            serde_json::to_value(agent_config).unwrap_or_default(),
        );
        data.insert(
            "history".into(),
            serde_json::json!(
                history
                    .iter()
                    .map(|m| serde_json::to_value(m).unwrap_or_default())
                    .collect::<Vec<_>>()
            ),
        );
        data.insert(
            "tool_schema_hash".into(),
            serde_json::json!(tool_schema_hash),
        );
        data.insert(
            "read_cache".into(),
            serde_json::to_value(read_cache).unwrap_or_default(),
        );
        data.insert(
            "metadata".into(),
            serde_json::to_value(metadata).unwrap_or_default(),
        );
        let json = serde_json::to_string_pretty(&serde_json::Value::Object(data))?;
        let temp = filepath.with_extension("tmp");
        fs::write(&temp, &json)?;
        fs::rename(&temp, &filepath)?;
        Ok(filepath.to_string_lossy().to_string())
    }

    pub fn load(&self, filepath: &Path) -> io::Result<serde_json::Value> {
        serde_json::from_str(&fs::read_to_string(filepath)?)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn list_sessions(&self) -> io::Result<Vec<HashMap<String, serde_json::Value>>> {
        let mut sessions = Vec::new();
        if !self.session_dir.exists() {
            return Ok(sessions);
        }
        for entry in fs::read_dir(&self.session_dir)? {
            let entry = entry?;
            if entry.path().extension().map_or(false, |e| e == "json") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                        let mut info = HashMap::with_capacity(5);
                        info.insert(
                            "filename".into(),
                            serde_json::json!(entry.file_name().to_string_lossy()),
                        );
                        info.insert(
                            "filepath".into(),
                            serde_json::json!(entry.path().to_string_lossy()),
                        );
                        info.insert(
                            "session_id".into(),
                            data.get("session_id").cloned().unwrap_or_default(),
                        );
                        info.insert(
                            "saved_at".into(),
                            data.get("saved_at").cloned().unwrap_or_default(),
                        );
                        info.insert(
                            "metadata".into(),
                            data.get("metadata").cloned().unwrap_or_default(),
                        );
                        sessions.push(info);
                    }
                }
            }
        }
        sessions.sort_by(|a, b| {
            b.get("saved_at")
                .and_then(|v| v.as_str())
                .cmp(&a.get("saved_at").and_then(|v| v.as_str()))
        });
        Ok(sessions)
    }

    pub fn delete(&self, name: &str) -> io::Result<bool> {
        let fp = self.session_dir.join(format!("{}.json", name));
        if fp.exists() {
            fs::remove_file(fp)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn check_config_consistency(
        saved: &HashMap<String, serde_json::Value>,
        current: &HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        let mut warnings: Vec<String> = Vec::new();
        if saved.get("llm_provider") != current.get("llm_provider") {
            warnings.push("LLM提供商变化".into());
        }
        if saved.get("llm_model") != current.get("llm_model") {
            warnings.push("模型变化".into());
        }
        let mut r = HashMap::with_capacity(2);
        r.insert("consistent".into(), serde_json::json!(warnings.is_empty()));
        r.insert("warnings".into(), serde_json::json!(warnings));
        r
    }

    pub fn compute_tool_hash(signature: &str) -> String {
        let mut h = Sha256::new();
        h.update(signature.as_bytes());
        //format!("{:x}", &h.finalize()[..16])
        format!("{:x}", &h.finalize())
    }
}
