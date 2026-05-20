use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TodoItem {
    pub content: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl TodoItem {
    pub fn new(content: impl Into<String>, status: impl Into<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        TodoItem {
            content: content.into(),
            status: status.into(),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TodoList {
    pub summary: String,
    pub todos: Vec<TodoItem>,
}

impl TodoList {
    pub fn new(summary: impl Into<String>) -> Self {
        TodoList {
            summary: summary.into(),
            todos: Vec::new(),
        }
    }
    pub fn get_in_progress(&self) -> Option<&TodoItem> {
        self.todos.iter().find(|t| t.status == "in_progress")
    }
    pub fn get_stats(&self) -> HashMap<String, usize> {
        let mut s = HashMap::new();
        s.insert("total".into(), self.todos.len());
        s.insert(
            "completed".into(),
            self.todos
                .iter()
                .filter(|t| t.status == "completed")
                .count(),
        );
        s.insert(
            "in_progress".into(),
            self.todos
                .iter()
                .filter(|t| t.status == "in_progress")
                .count(),
        );
        s
    }
}

pub struct TodoWriteTool {
    persistence_dir: PathBuf,
    current_todos: RwLock<TodoList>,
}

impl TodoWriteTool {
    pub fn new(project_root: &str, persistence_dir: &str) -> Self {
        let dir = PathBuf::from(project_root).join(persistence_dir);
        let _ = fs::create_dir_all(&dir);
        TodoWriteTool {
            persistence_dir: dir,
            current_todos: RwLock::new(TodoList::new("")),
        }
    }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }
    fn description(&self) -> &str {
        "管理任务列表，保持单线程专注"
    }
    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        let action = parameters
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("create");
        if action == "clear" {
            *self.current_todos.write() = TodoList::new("");
            return ToolResponse::success("✅ 任务列表已清空", HashMap::new());
        }
        let summary = parameters
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let todos_data = parameters
            .get("todos")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut todos = Vec::new();
        for item in &todos_data {
            let content = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let status = item
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("pending")
                .to_string();
            if !["pending", "in_progress", "completed"].contains(&status.as_str()) {
                return ToolResponse::error(
                    "INVALID_PARAM",
                    "status必须是pending/in_progress/completed",
                );
            }
            todos.push(TodoItem::new(content, status));
        }
        let ip_count = todos.iter().filter(|t| t.status == "in_progress").count();
        if ip_count > 1 {
            return ToolResponse::error(
                "INVALID_PARAM",
                &format!("最多1个in_progress，当前{}个", ip_count),
            );
        }
        *self.current_todos.write() = TodoList { summary, todos };
        ToolResponse::success("✅ 任务列表已更新", HashMap::new())
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::optional("summary", "string", "总体任务描述")
                .with_default(serde_json::json!("")),
            ToolParameter::optional("todos", "array", "待办事项列表")
                .with_default(serde_json::json!([])),
            ToolParameter::optional("action", "string", "操作类型：create/update/clear")
                .with_default(serde_json::json!("create")),
        ]
    }
}
