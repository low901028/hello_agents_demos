use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use chrono::Utc;

use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;

/// 待办事项
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TodoItem {
    pub content: String,
    pub status: String, // pending | in_progress | completed
    pub created_at: String,
    pub updated_at: String,
}

/// 待办列表
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TodoList {
    pub summary: String,
    pub todos: Vec<TodoItem>,
}

impl TodoList {
    pub fn get_in_progress(&self) -> Option<&TodoItem> {
        self.todos.iter().find(|t| t.status == "in_progress")
    }

    pub fn get_stats(&self) -> serde_json::Value {
        let total = self.todos.len();
        let completed = self.todos.iter().filter(|t| t.status == "completed").count();
        let in_progress = self.todos.iter().filter(|t| t.status == "in_progress").count();
        let pending = total - completed - in_progress;

        serde_json::json!({
            "total": total,
            "completed": completed,
            "in_progress": in_progress,
            "pending": pending,
        })
    }
}

/// TodoWrite 工具
pub struct TodoWriteTool {
    persistence_dir: PathBuf,
    current_todos: TodoList,
}

impl TodoWriteTool {
    pub fn new(project_root: impl Into<PathBuf>, persistence_dir: impl Into<PathBuf>) -> Self {
        let dir = persistence_dir.into();
        fs::create_dir_all(&dir).ok();

        Self {
            persistence_dir: dir,
            current_todos: TodoList {
                summary: String::new(),
                todos: Vec::new(),
            },
        }
    }

    fn generate_recap(&self) -> String {
        let stats = self.current_todos.get_stats();
        let total = stats["total"].as_u64().unwrap_or(0);
        let completed = stats["completed"].as_u64().unwrap_or(0);

        if total == 0 {
            return "📋 [0/0] 无活动任务".to_string();
        }

        let mut parts = vec![format!("📋 [{}/{}]", completed, total)];

        if let Some(in_progress) = self.current_todos.get_in_progress() {
            parts.push(format!("进行中: {}", in_progress.content));
        }

        let pending: Vec<&str> = self.current_todos.todos
            .iter()
            .filter(|t| t.status == "pending")
            .take(3)
            .map(|t| t.content.as_str())
            .collect();

        if !pending.is_empty() {
            parts.push(format!("待处理: {}", pending.join("; ")));
        }

        if completed == total && total > 0 {
            return format!("✅ [{}/{}] 所有任务已完成！", completed, total);
        }

        parts.join(". ")
    }

    fn validate_todos(&self, todos: &[serde_json::Value]) -> Result<(), String> {
        let in_progress = todos.iter().filter(|t| {
            t.get("status").and_then(|s| s.as_str()) == Some("in_progress")
        }).count();

        if in_progress > 1 {
            return Err(format!("最多只能有 1 个 in_progress 任务，当前有 {} 个", in_progress));
        }

        for (i, todo) in todos.iter().enumerate() {
            let content = todo.get("content").and_then(|c| c.as_str()).unwrap_or("");
            let status = todo.get("status").and_then(|s| s.as_str()).unwrap_or("");

            if content.is_empty() {
                return Err(format!("第 {} 个任务的 content 不能为空", i + 1));
            }

            if !["pending", "in_progress", "completed"].contains(&status) {
                return Err(format!("第 {} 个任务的 status 无效: {}", i + 1, status));
            }
        }

        Ok(())
    }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str { "TodoWrite" }

    fn description(&self) -> &str {
        "管理任务列表，保持单线程专注。每次提交完整列表，最多 1 个 in_progress 任务。"
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("summary", "string", "总体任务描述", false).with_default(serde_json::json!("")),
            ToolParameter::new("todos", "array", "待办事项列表（JSON 数组）", false).with_default(serde_json::json!([])),
            ToolParameter::new("action", "string", "操作类型：create|update|clear", false).with_default(serde_json::json!("create")),
        ]
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let action = parameters.get("action").and_then(|v| v.as_str()).unwrap_or("create");

        if action == "clear" {
            return ToolResponse::success("✅ 任务列表已清空")
                .with_data("stats", serde_json::json!({"total": 0, "completed": 0, "in_progress": 0, "pending": 0}));
        }

        let todos_data = parameters.get("todos").cloned().unwrap_or(serde_json::json!([]));
        let todos: Vec<serde_json::Value> = if todos_data.is_string() {
            serde_json::from_str(todos_data.as_str().unwrap_or("[]")).unwrap_or_default()
        } else if todos_data.is_array() {
            todos_data.as_array().cloned().unwrap_or_default()
        } else {
            return ToolResponse::error(ToolErrorCode::INVALID_PARAM, "todos 必须是数组或 JSON 字符串");
        };

        // 验证
        if let Err(msg) = self.validate_todos(&todos) {
            return ToolResponse::error(ToolErrorCode::INVALID_PARAM, msg);
        }

        // 构建新列表（因为 self 是 &self，这里返回摘要信息）
        let now = Utc::now().to_rfc3339();
        let summary = parameters.get("summary").and_then(|v| v.as_str()).unwrap_or("");

        let total = todos.len();
        let completed = todos.iter().filter(|t| t.get("status").and_then(|s| s.as_str()) == Some("completed")).count();

        let recap = format!("📋 [{}/{}] 任务列表已更新: {}", completed, total, summary);

        ToolResponse::success(recap)
            .with_data("summary", summary)
            .with_data("stats", serde_json::json!({
                "total": total,
                "completed": completed,
            }))
    }
}