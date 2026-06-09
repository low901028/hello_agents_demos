//! =====================================================
//! SessionStore - 会话持久化存储
//!
//! 职责：
//! - 保存会话到文件（原子写入）
//! - 从文件恢复会话
//! - 环境一致性检查
//! - 会话列表管理
//! =====================================================

use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{self};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// 会话存储器
///
///     功能：
///     - 保存会话到 JSON 文件
///     - 从文件恢复会话
///     - 环境一致性检查
///     - 原子写入保证数据完整性
///
///     用法示例：
///     ```rust
///     let store = SessionStore(session_dir="memory/sessions")
///
///     # 保存会话
///     let filepath = store.save(
///         agent_config={"name": "assistant", "llm_model": "deepseek-v4-flash"},
///         history=[...],
///         tool_schema_hash="abc123",
///         read_cache={},
///         metadata={"total_tokens": 1000}
///     )
///
///     # 加载会话
///     let session_data = store.load(filepath)
///
///     # 列出所有会话
///     let sessions = store.list_sessions()
///     ```
///
#[derive(Debug, Clone)]
pub struct SessionStore {
    session_dir: PathBuf,
}

impl SessionStore {
    /// 初始化会话存储器
    ///
    /// @Args:
    ///   session_dir: 会话文件保存目录
    ///
    pub fn new(session_dir: &str) -> io::Result<Self> {
        let dir = PathBuf::from(session_dir);
        fs::create_dir_all(&dir)?;
        Ok(SessionStore { session_dir: dir })
    }

    /// 生成唯一的会话 ID
    ///
    ///  格式：s-{timestamp}-{uuid}
    ///  Returns:
    ///      会话 ID
    ///
    fn generate_session_id(&self) -> String {
        format!(
            "s-{}-{}",
            Utc::now().format("%Y%m%d-%H%M%S"),
            &Uuid::new_v4().to_string()[..8]
        )
    }

    /// 保存会话
    ///
    ///         Args:
    ///             agent_config: Agent 配置信息
    ///             history: 消息历史列表
    ///             tool_schema_hash: 工具 Schema 哈希值
    ///             read_cache: Read 工具的元数据缓存
    ///             metadata: 会话元数据（tokens、steps、duration 等）
    ///             session_name: 自定义会话名称（可选）
    ///
    ///         Returns:
    ///             保存的文件路径
    pub fn save(
        &self,
        agent_config: &HashMap<String, serde_json::Value>,
        history: &[crate::core::message::Message],
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
            serde_json::json!(metadata
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or(format!("{}", chrono::Local::now().naive_local().format("%Y-%m-%dT%H:%M:%S%.f")).as_str())),
        );

        data.insert(
            "saved_at".into(),
            serde_json::json!(format!("{}", chrono::Local::now().naive_local().format("%Y-%m-%dT%H:%M:%S%.f")).as_str()),
        );
        data.insert(
            "agent_config".into(),
            serde_json::to_value(agent_config).unwrap_or_default(),
        );
        data.insert(
            "history".into(),
            serde_json::json!(history
                .iter()
                .map(|m| {
                    serde_json::to_value(m).unwrap_or_default()
                })
                .collect::<Vec<_>>()),
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
        // 原子写入（临时文件 + 重命名）
        let temp = filepath.with_extension("tmp");
        fs::write(&temp, &json)?;
        fs::rename(&temp, &filepath)?;
        Ok(filepath.to_string_lossy().to_string())
    }

    /// 加载会话
    ///
    ///         Args:
    ///             filepath: 会话文件路径
    ///
    ///         Returns:
    ///             会话数据字典
    ///
    ///         Raises:
    ///             FileNotFoundError: 文件不存在
    ///             json.JSONDecodeError: 文件格式错误
    pub fn load(&self, filepath: &Path) -> io::Result<serde_json::Value> {
        serde_json::from_str(&fs::read_to_string(filepath)?)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// 列出所有会话
    ///
    ///         Returns:
    ///             会话信息列表，按保存时间倒序排列
    ///
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

    /// 删除会话
    ///
    ///         Args:
    ///             session_name: 会话名称（不含 .json 后缀）
    ///
    ///         Returns:
    ///             是否删除成功
    pub fn delete(&self, name: &str) -> io::Result<bool> {
        let fp = self.session_dir.join(format!("{}.json", name));
        if fp.exists() {
            fs::remove_file(fp)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 检查配置一致性
    ///
    ///         Args:
    ///             saved_config: 保存的配置
    ///             current_config: 当前配置
    ///
    ///         Returns:
    ///             检查结果字典，包含 warnings 列表
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

    /// 检查工具 Schema 一致性
    ///
    ///         Args:
    ///             saved_hash: 保存的工具 Schema 哈希
    ///             current_hash: 当前工具 Schema 哈希
    ///
    ///         Returns:
    ///             检查结果字典
    ///
    pub fn check_tool_schema_consistency(
        self,
        saved_hash: &str,
        current_hash: &str
    ) -> HashMap<String, serde_json::Value> {
        let changed = saved_hash != current_hash;

        let mut r = HashMap::with_capacity(4);
        r.insert("changed".into(), serde_json::json!(changed));
        r.insert("saved_hash".into(), serde_json::json!(saved_hash));
        r.insert("current_hash".into(), serde_json::json!(current_hash));
        r.insert("recommendation".into(), serde_json::json!(if changed {"建议重新读取文件"} else {"可以安全恢复"}));
        r
    }

    ///
    pub fn compute_tool_hash(signature: &str) -> String {
        let mut h = Sha256::new();
        h.update(signature.as_bytes());
        let result = &h.finalize();
        let hex_string = hex::encode(&result.0[..16]);
        hex_string
    }
}
