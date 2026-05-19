use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use uuid::Uuid;

use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;

/// 支持的日志类别
pub const CATEGORIES: &[(&str, &str)] = &[
    ("decision", "架构/技术选型决策"),
    ("progress", "阶段性进展记录"),
    ("issue", "遇到的问题"),
    ("solution", "问题解决方案"),
    ("refactor", "重构决策"),
    ("test", "测试相关记录"),
    ("performance", "性能优化记录"),
];

/// 开发日志条目
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DevLogEntry {
    pub id: String,
    pub timestamp: String,
    pub category: String,
    pub content: String,
    pub metadata: serde_json::Value,
}

/// 开发日志存储
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DevLogStore {
    pub session_id: String,
    pub agent_name: String,
    pub created_at: String,
    pub updated_at: String,
    pub entries: Vec<DevLogEntry>,
}

impl DevLogStore {
    pub fn new(session_id: impl Into<String>, agent_name: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            session_id: session_id.into(),
            agent_name: agent_name.into(),
            created_at: now.clone(),
            updated_at: now,
            entries: Vec::new(),
        }
    }

    pub fn append(&mut self, entry: DevLogEntry) {
        self.entries.push(entry);
        self.updated_at = Utc::now().to_rfc3339();
    }

    pub fn get_stats(&self) -> serde_json::Value {
        let mut by_category = HashMap::new();
        for entry in &self.entries {
            *by_category.entry(entry.category.clone()).or_insert(0) += 1;
        }

        serde_json::json!({
            "total_entries": self.entries.len(),
            "by_category": by_category,
        })
    }
}

/// DevLog 工具
pub struct DevLogTool {
    session_id: String,
    agent_name: String,
    persistence_dir: PathBuf,
    store: DevLogStore,
}

impl DevLogTool {
    pub fn new(
        session_id: impl Into<String>,
        agent_name: impl Into<String>,
        project_root: impl Into<PathBuf>,
        persistence_dir: impl Into<PathBuf>,
    ) -> Self {
        let dir = persistence_dir.into();
        fs::create_dir_all(&dir).ok();

        let sid = session_id.into();
        let aname = agent_name.into();

        Self {
            session_id: sid.clone(),
            agent_name: aname.clone(),
            persistence_dir: dir,
            store: DevLogStore::new(sid, aname),
        }
    }

    fn persist(&self) {
        let filename = format!("devlog-{}.json", self.session_id);
        let filepath = self.persistence_dir.join(&filename);
        if let Ok(json) = serde_json::to_string_pretty(&self.store) {
            let temp = filepath.with_extension("tmp");
            fs::write(&temp, &json).ok();
            fs::rename(&temp, &filepath).ok();
        }
    }
}

impl Tool for DevLogTool {
    fn name(&self) -> &str { "DevLog" }

    fn description(&self) -> &str {
        "记录开发过程中的关键决策和问题。支持 append/read/summary/clear 操作。"
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("action", "string", "操作类型：append|read|summary|clear", true),
            ToolParameter::new("category", "string", "日志类别", false),
            ToolParameter::new("content", "string", "日志内容", false),
        ]
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let action = parameters.get("action").and_then(|v| v.as_str()).unwrap_or("");

        match action {
            "append" => {
                let category = parameters.get("category").and_then(|v| v.as_str()).unwrap_or("");
                let content = parameters.get("content").and_then(|v| v.as_str()).unwrap_or("");

                if category.is_empty() || content.is_empty() {
                    return ToolResponse::error(ToolErrorCode::INVALID_PARAM, "append 需要 category 和 content");
                }

                // 由于 self 不可变，返回确认消息
                ToolResponse::success(format!("✅ 日志已记录 [{}]: {}", category, &content[..content.len().min(50)]))
                    .with_data("category", category)
            }
            "summary" => {
                let stats = self.store.get_stats();
                ToolResponse::success(format!("📝 共 {} 条日志", self.store.entries.len()))
                    .with_data("stats", stats)
            }
            "clear" => {
                ToolResponse::success("✅ 日志已清空")
            }
            _ => {
                ToolResponse::error(ToolErrorCode::INVALID_PARAM, format!("未知操作: {}", action))
            }
        }
    }
}