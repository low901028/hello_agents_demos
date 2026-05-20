use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DevLogEntry {
    pub id: String,
    pub timestamp: String,
    pub category: String,
    pub content: String,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl DevLogEntry {
    pub fn create(category: &str, content: &str) -> Self {
        DevLogEntry {
            id: format!("log-{}", &Uuid::new_v4().to_string()[..8]),
            timestamp: Utc::now().to_rfc3339(),
            category: category.to_string(),
            content: content.to_string(),
            metadata: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DevLogStore {
    pub session_id: String,
    pub agent_name: String,
    pub entries: Vec<DevLogEntry>,
}

impl DevLogStore {
    pub fn create(session_id: &str, agent_name: &str) -> Self {
        DevLogStore {
            session_id: session_id.to_string(),
            agent_name: agent_name.to_string(),
            entries: Vec::new(),
        }
    }
    pub fn append(&mut self, entry: DevLogEntry) {
        self.entries.push(entry);
    }
    pub fn get_stats(&self) -> HashMap<String, usize> {
        let mut s = HashMap::new();
        s.insert("total".into(), self.entries.len());
        s
    }
}

pub struct DevLogTool {
    session_id: String,
    agent_name: String,
    persistence_dir: PathBuf,
    store: RwLock<DevLogStore>,
}

impl DevLogTool {
    pub fn new(
        session_id: &str,
        agent_name: &str,
        project_root: &str,
        persistence_dir: &str,
    ) -> Self {
        let dir = PathBuf::from(project_root).join(persistence_dir);
        let _ = fs::create_dir_all(&dir);
        DevLogTool {
            session_id: session_id.to_string(),
            agent_name: agent_name.to_string(),
            persistence_dir: dir,
            store: RwLock::new(DevLogStore::create(session_id, agent_name)),
        }
    }
}

impl Tool for DevLogTool {
    fn name(&self) -> &str {
        "DevLog"
    }
    fn description(&self) -> &str {
        "记录开发过程中的关键决策和问题"
    }
    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        let action = parameters
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("append");
        match action {
            "append" => {
                let category = parameters
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let content = parameters
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if category.is_empty() || content.is_empty() {
                    return ToolResponse::error("INVALID_PARAM", "category和content不能为空");
                }
                self.store
                    .write()
                    .append(DevLogEntry::create(category, content));
                ToolResponse::success("✅ 日志已记录", HashMap::new())
            }
            "summary" => {
                let stats = self.store.read().get_stats();
                let mut data = HashMap::new();
                for (k, v) in stats {
                    data.insert(k, serde_json::json!(v));
                }
                ToolResponse::success("📝 日志摘要", data)
            }
            "clear" => {
                *self.store.write() = DevLogStore::create(&self.session_id, &self.agent_name);
                ToolResponse::success("✅ 日志已清空", HashMap::new())
            }
            _ => ToolResponse::error("INVALID_PARAM", &format!("未知操作: {}", action)),
        }
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("action", "string", "操作类型：append/summary/clear"),
            ToolParameter::optional("category", "string", "日志类别"),
            ToolParameter::optional("content", "string", "日志内容"),
        ]
    }
}
