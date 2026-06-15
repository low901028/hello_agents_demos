use crate::core::traits::tool::Tool;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::ToolResponse;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Mutex;

pub struct TodoWriteTool {
    current: Mutex<TodoList>,
}

#[derive(Clone)]
struct TodoItem {
    content: String,
    status: String,
    created_at: String,
}

#[derive(Clone)]
struct TodoList {
    summary: String,
    todos: Vec<TodoItem>,
}

impl TodoList {
    fn new() -> Self {
        Self {
            summary: String::new(),
            todos: vec![],
        }
    }
}

impl TodoWriteTool {
    pub fn new() -> Self {
        Self {
            current: Mutex::new(TodoList::new()),
        }
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }
    fn description(&self) -> &str {
        "管理任务列表，保持单线程专注"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "description": "操作类型：create|update|clear", "default": "create" },
                "summary": { "type": "string", "description": "总体任务描述" },
                "todos": { "type": "array", "description": "任务列表" }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let action = args["action"].as_str().unwrap_or("create");
        let mut list = self.current.lock().unwrap();
        match action {
            "clear" => {
                *list = TodoList::new();
                Ok(ToolResponse::success("任务列表已清空"))
            }
            _ => {
                let summary = args["summary"].as_str().unwrap_or("").to_string();
                let todos = args["todos"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|v| TodoItem {
                                content: v["content"].as_str().unwrap_or("").to_string(),
                                status: v["status"].as_str().unwrap_or("pending").to_string(),
                                created_at: v
                                    .get("created_at")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                *list = TodoList { summary, todos };
                Ok(ToolResponse::success(format!(
                    "任务列表已更新，共 {} 项",
                    list.todos.len()
                )))
            }
        }
    }
}
