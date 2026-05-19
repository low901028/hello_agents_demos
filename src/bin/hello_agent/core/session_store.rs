use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// 会话存储器
#[derive(Clone)]
pub struct SessionStore {
    session_dir: PathBuf,
}

impl SessionStore {
    pub fn new(session_dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = session_dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { session_dir: dir })
    }

    fn generate_session_id(&self) -> String {
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
        let suffix = uuid::Uuid::new_v4().to_string()[..8].to_string();
        format!("s-{}-{}", timestamp, suffix)
    }

    /// 保存会话
    pub fn save(
        &self,
        agent_config: &serde_json::Value,
        history: &[serde_json::Value],
        tool_schema_hash: &str,
        read_cache: &serde_json::Value,
        metadata: &serde_json::Value,
        session_name: Option<&str>,
    ) -> Result<String> {
        let session_id = self.generate_session_id();

        let filename = if let Some(name) = session_name {
            format!("{}.json", name)
        } else {
            format!("session-{}.json", session_id)
        };
        let filepath = self.session_dir.join(&filename);

        let session_data = serde_json::json!({
            "session_id": session_id,
            "created_at": metadata.get("created_at").unwrap_or(&serde_json::json!(Utc::now().to_rfc3339())),
            "saved_at": Utc::now().to_rfc3339(),
            "agent_config": agent_config,
            "history": history,
            "tool_schema_hash": tool_schema_hash,
            "read_cache": read_cache,
            "metadata": metadata,
        });

        // 原子写入
        let temp_path = filepath.with_extension("tmp");
        fs::write(&temp_path, serde_json::to_string_pretty(&session_data)?)?;
        fs::rename(&temp_path, &filepath)?;

        Ok(filepath.to_string_lossy().to_string())
    }

    /// 加载会话
    pub fn load(&self, filepath: &str) -> Result<serde_json::Value> {
        let content = fs::read_to_string(filepath)?;
        let data: serde_json::Value = serde_json::from_str(&content)?;
        Ok(data)
    }

    /// 列出所有会话
    pub fn list_sessions(&self) -> Result<Vec<serde_json::Value>> {
        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.session_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                        sessions.push(serde_json::json!({
                            "filename": path.file_name().unwrap().to_string_lossy(),
                            "filepath": path.to_string_lossy(),
                            "session_id": data.get("session_id"),
                            "created_at": data.get("created_at"),
                            "saved_at": data.get("saved_at"),
                            "metadata": data.get("metadata"),
                        }));
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

    /// 删除会话
    pub fn delete(&self, session_name: &str) -> bool {
        let filepath = self.session_dir.join(format!("{}.json", session_name));
        if filepath.exists() {
            fs::remove_file(filepath).is_ok()
        } else {
            false
        }
    }

    /// 检查配置一致性
    pub fn check_config_consistency(
        &self,
        saved_config: &serde_json::Value,
        current_config: &serde_json::Value,
    ) -> serde_json::Value {
        let mut warnings = Vec::new();

        let checks = [
            ("llm_provider", "LLM 提供商"),
            ("llm_model", "模型"),
            ("max_steps", "最大步数"),
        ];

        for (key, label) in checks {
            let saved = saved_config.get(key).and_then(|v| v.as_str());
            let current = current_config.get(key).and_then(|v| v.as_str());
            if saved != current {
                warnings.push(format!(
                    "{}变化: {:?} → {:?}",
                    label,
                    saved.unwrap_or("unknown"),
                    current.unwrap_or("unknown")
                ));
            }
        }

        serde_json::json!({
            "consistent": warnings.is_empty(),
            "warnings": warnings,
        })
    }

    /// 检查工具 Schema 一致性
    pub fn check_tool_schema_consistency(&self, saved_hash: &str, current_hash: &str) -> serde_json::Value {
        let changed = saved_hash != current_hash;
        serde_json::json!({
            "changed": changed,
            "saved_hash": saved_hash,
            "current_hash": current_hash,
            "recommendation": if changed { "建议重新读取文件" } else { "可以安全恢复" },
        })
    }
}