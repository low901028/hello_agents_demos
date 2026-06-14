//! todo_write.rs
//! TodoWrite 进度管理工具
///
/// 提供任务列表管理能力，强制单线程专注，避免任务切换。
///
/// 特性：
/// - 声明式覆盖（每次提交完整列表）
/// - 单线程强制（最多 1 个 in_progress）
/// - 自动 Recap 生成
/// - 持久化到 memory/todos/

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::Local;
use serde_json::{json, Value};

use crate::core::traits::tool::{Tool, ToolParameter};
use crate::core::types::exceptions::HelloAgentException;
use crate::tools::error::ToolErrorCode;
use crate::tools::response::ToolResponse;
use crate::tools::tool_base::{ ToolBase};

// ------------------------------------------------------------
// TodoItem
// ------------------------------------------------------------
/// 单个待办事项
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TodoItem {
    content: String,
    status: String,  // "pending" | "in_progress" | "completed"
    created_at: String,
    updated_at: String,
}

impl TodoItem {
    fn new(content: String, status: String, created_at: String, updated_at: String) -> Self {
        let update_at = if updated_at.is_empty() {
            created_at.clone()
        } else {
            updated_at
        };

        TodoItem {
            content,
            status,
            created_at,
            updated_at: update_at,
        }
    }
}

// ------------------------------------------------------------
// TodoList
// ------------------------------------------------------------
/// 待办列表
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TodoList {
    summary: String,
    todos: Vec<TodoItem>,
}

impl TodoList {
    fn get_in_progress(&self) -> Option<&TodoItem> {
        self.todos.iter().find(|t| t.status == "in_progress")
    }

    fn get_pending(&self, limit: usize) -> Vec<&TodoItem> {
        self.todos
            .iter()
            .filter(|t| t.status == "pending")
            .take(limit)
            .collect()
    }

    fn get_completed(&self) -> Vec<&TodoItem> {
        self.todos.iter().filter(|t| t.status == "completed").collect()
    }

    fn get_stats(&self) -> Value {
        let total = self.todos.len();
        let completed = self.todos.iter().filter(|t| t.status == "completed").count();
        let in_progress = self.todos.iter().filter(|t| t.status == "in_progress").count();
        let pending = total - completed - in_progress;

        json!({
            "total": total,
            "completed": completed,
            "in_progress": in_progress,
            "pending": pending,
        })
    }
}

// ------------------------------------------------------------
// TodoWriteTool
// ------------------------------------------------------------
/// 待办事项工具
///
///     特性：
///     - 声明式覆盖（每次提交完整列表）
///     - 单线程强制（最多 1 个 in_progress）
///     - 自动 Recap 生成
///     - 持久化到文件
pub struct TodoWriteTool {
    base: ToolBase,
    persistence_dir: PathBuf,
    current_todos: Mutex<TodoList>,
}

impl TodoWriteTool {
    pub fn new(project_root: impl Into<PathBuf>, persistence_dir: impl Into<PathBuf>) -> Self {
        let persistence_dir = persistence_dir.into();
        fs::create_dir_all(&persistence_dir).ok();

        Self {
            base: ToolBase::new(
                "TodoWrite",
                "管理任务列表，保持单线程专注。\n\n特性：\n- 每次提交完整列表（声明式）\n- 最多 1 个任务标记为 in_progress\n- 自动生成 Recap 保持上下文精简\n- 自动保存到 memory/todos/\n\n使用场景：\n- 开始复杂任务时创建任务列表\n- 跟踪进度，避免遗漏\n- 多轮对话中保持状态\n\n参数：\n- summary: 总体任务描述（可选）\n- todos: 待办事项列表（JSON 数组）\n- action: 操作类型（create/update/clear，默认 create）",
                false,
            ),
            persistence_dir,
            current_todos: Mutex::new(TodoList { summary: String::new(), todos: Vec::new() }),
        }
    }

    /// 从文件加载任务列表
    pub fn load_todos(&self, filepath: &Path) -> Result<(), HelloAgentException> {
        let content = fs::read_to_string(filepath)
            .map_err(|e| HelloAgentException::ToolException(format!("读取文件失败: {}", e)))?;
        let data: Value = serde_json::from_str(&content)
            .map_err(|e| HelloAgentException::SerializationException(e.to_string()))?;

        let todos: Vec<TodoItem> = data["todos"]
            .as_array()
            .ok_or_else(|| HelloAgentException::ToolException("todos 字段缺失".into()))?
            .iter()
            .map(|t| {
                TodoItem {
                    content: t["content"].as_str().unwrap_or("").to_string(),
                    status: t["status"].as_str().unwrap_or("").to_string(),
                    created_at: t["created_at"].as_str().unwrap_or("").to_string(),
                    updated_at: t.get("updated_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| t["created_at"].as_str().unwrap_or(""))
                        .to_string(),
                }
            })
            .collect();

        let summary = data["summary"].as_str().unwrap_or("").to_string();

        let mut list = self.current_todos.lock().unwrap();
        *list = TodoList { summary, todos };
        Ok(())
    }

    // ---------- 内部辅助方法 ----------
    /// 验证 todos 约束
    ///
    /// Returns:
    ///    {"valid": bool, "message": str}
    fn validate_todos(todos_data: &[Value]) -> Result<(), String> {
        let mut in_progress_count = 0;

        for (i, todo) in todos_data.iter().enumerate() {
            let obj = todo.as_object().ok_or_else(|| format!("第 {} 个任务必须是对象", i + 1))?;

            let content = obj.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let status = obj.get("status").and_then(|v| v.as_str()).unwrap_or("");

            if content.trim().is_empty() {
                return Err(format!("第 {} 个任务的 content 不能为空", i + 1));
            }

            if !["pending", "in_progress", "completed"].contains(&status) {
                return Err(format!(
                    "第 {} 个任务的 status 必须是 pending/in_progress/completed",
                    i + 1
                ));
            }

            if status == "in_progress" {
                in_progress_count += 1;
            }
        }

        if in_progress_count > 1 {
            return Err(format!(
                "最多只能有 1 个 in_progress 任务，当前有 {} 个",
                in_progress_count
            ));
        }

        Ok(())
    }

    /// 生成 Recap 文本
    ///
    ///   格式：[2/5] In progress: xxx. Pending: yyy; zzz.
    ///
    fn generate_recap(list: &TodoList) -> String {
        let stats = list.get_stats();
        let total = stats["total"].as_u64().unwrap_or(0);
        let completed = stats["completed"].as_u64().unwrap_or(0);

        if total == 0 {
            return "📋 [0/0] 无活动任务".to_string();
        }

        let mut parts = vec![format!("📋 [{}/{}]", completed, total)];

        if let Some(in_progress) = list.get_in_progress() {
            parts.push(format!("进行中: {}", in_progress.content));
        }

        let pending = list.get_pending(3);
        if !pending.is_empty() {
            let pending_texts: Vec<&str> = pending.iter().map(|t| t.content.as_str()).collect();
            parts.push(format!("待处理: {}", pending_texts.join("; ")));
        }

        let pending_total = stats["pending"].as_u64().unwrap_or(0);
        if pending_total > 3 {
            parts.push(format!("还有 {} 个...", pending_total - 3));
        }

        if completed == total && total > 0 {
            return format!("✅ [{}/{}] 所有任务已完成！", completed, total);
        }

        parts.join(". ")
    }

    fn persist_todos(list: &TodoList, dir: &Path) -> std::io::Result<()> {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let filename = format!("todoList-{}.json", timestamp);
        let filepath = dir.join(filename);
        let temp_path = filepath.with_extension("tmp");

        let data = json!({
            "summary": list.summary,
            "todos": list.todos.iter().map(|t| {
                json!({
                    "content": t.content,
                    "status": t.status,
                    "created_at": t.created_at,
                    "updated_at": t.updated_at,
                })
            }).collect::<Vec<_>>(),
            "created_at": Local::now().to_rfc3339(),
            "stats": list.get_stats(),
        });

        let mut file = fs::File::create(&temp_path)?;
        file.write_all(serde_json::to_string_pretty(&data).unwrap().as_bytes())?;
        fs::rename(temp_path, filepath)?;
        Ok(())
    }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let action = parameters
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("create")
            .to_string();

        if action == "clear" {
            let mut list = self.current_todos.lock().unwrap();
            *list = TodoList { summary: String::new(), todos: vec![] };
            return Ok(ToolResponse::success(
                "✅ 任务列表已清空".to_string(),
                Some(json!({
                    "action": "clear",
                    "summary": "",
                    "stats": {"total": 0, "completed": 0, "in_progress": 0, "pending": 0},
                })),
                None,
                None,
            ));
        }

        let todos_data = parameters.get("todos").cloned().unwrap_or(json!([]));
        let todos_array = if todos_data.is_string() {
            let s = todos_data.as_str().unwrap();
            serde_json::from_str::<Value>(s)
                .map_err(|e| HelloAgentException::SerializationException(e.to_string()))?
                .as_array()
                .cloned()
                .unwrap_or_default()
        } else {
            todos_data.as_array().cloned().unwrap_or_default()
        };

        // 验证
        if let Err(msg) = Self::validate_todos(&todos_array) {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                &msg,
                None,
                None,
            ));
        }

        let now = Local::now().to_rfc3339();
        let todos: Vec<TodoItem> = todos_array
            .iter()
            .map(|t| {
                let content = t["content"].as_str().unwrap_or("").to_string();
                let status = t["status"].as_str().unwrap_or("").to_string();
                let created_at = t["created_at"].as_str().unwrap_or(&now).to_string();
                TodoItem::new(content, status, created_at, now.clone())
            })
            .collect();

        let summary = parameters
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        {
            let mut list = self.current_todos.lock().unwrap();
            *list = TodoList { summary, todos };
        }

        let list = self.current_todos.lock().unwrap();
        let recap = Self::generate_recap(&list);
        let stats = list.get_stats();
        let summary = list.summary.clone();
        Self::persist_todos(&list, &self.persistence_dir)
            .map_err(|e| HelloAgentException::IoException(e))?;

        Ok(ToolResponse::success(
            recap,
            Some(json!({
                "action": action,
                "summary": summary,
                "stats": stats,
            })),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new(
                "summary",
                "string",
                "总体任务描述（简短，1-2 句话）",
                false,
                Some(Value::String("".into())),
            ),
            ToolParameter::new(
                "todos",
                "array",
                "待办事项列表（JSON 数组）\n\n格式：[\n  {\"content\": \"任务1\", \"status\": \"pending\"},\n  {\"content\": \"任务2\", \"status\": \"in_progress\"},\n  {\"content\": \"任务3\", \"status\": \"completed\"}\n]\n\n规则：\n- status 只能是：pending, in_progress, completed\n- 最多 1 个任务可以标记为 in_progress\n- 每次提交完整列表（声明式）",
                false,
                Some(json!([])),
            ),
            ToolParameter::new(
                "action",
                "string",
                "操作类型：create|update|clear（默认 create）",
                false,
                Some(Value::String("create".into())),
            ),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        let current = self.current_todos.lock().unwrap().clone();
        Box::new(Self {
            base: self.base.clone(),
            persistence_dir: self.persistence_dir.clone(),
            current_todos: Mutex::new(current),
        })
    }

    fn validate_parameters(&self, parameters: &Value) -> bool {
        let required = self.get_parameters().iter().filter(|p| p.required).map(|p| p.name.clone()).collect::<Vec<_>>();
        self.base.validate_parameters(parameters, &required)
    }
    fn to_openai_schema(&self) -> Value {
        self.base.to_openai_schema(&self.get_parameters())
    }
}

// ------------------------------------------------------------
// 测试
// ------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use crate::tools::registry::{global_registry, ToolRegistry};
    use crate::tools::response::ToolStatus;

    fn set_up_registry() -> std::sync::MutexGuard<'static, ToolRegistry>{
        // 全局注册中心
        let guard = global_registry();
        let registry = guard.lock().unwrap();

        registry
    }

    fn test_tool() -> TodoWriteTool {
        let tmp = env::temp_dir().join("todo_test");
        TodoWriteTool::new(".", tmp)
    }

    #[test]
    fn test_create_todo_list() {
        let tool = test_tool();
        let params = json!({
            "action": "create",
            "summary": "实现登录功能",
            "todos": [
                {"content": "设计数据库", "status": "completed"},
                {"content": "编写API", "status": "in_progress"},
                {"content": "写前端", "status": "pending"},
            ]
        });

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = params;
        let resp = registry.execute_tool("TodoWrite", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================

        //let resp = tool.run(params).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        let data = resp.data;
        assert_eq!(data["summary"], "实现登录功能");
        let stats = &data["stats"];
        assert_eq!(stats["total"], 3);
        assert_eq!(stats["completed"], 1);
        assert_eq!(stats["in_progress"], 1);
        assert_eq!(stats["pending"], 1);
    }

    #[test]
    fn test_too_many_in_progress() {
        let tool = test_tool();
        let params = json!({
            "todos": [
                {"content": "a", "status": "in_progress"},
                {"content": "b", "status": "in_progress"},
            ]
        });

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = params;
        let resp = registry.execute_tool("TodoWrite", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================

        // let resp = tool.run(params).unwrap();
        assert_eq!(resp.status, ToolStatus::Error);
        assert!(resp.error_info.unwrap().message.contains("最多只能有 1 个 in_progress"));
    }

    #[test]
    fn test_clear() {
        let tool = test_tool();

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = json!({
            "todos": [{"content": "test", "status": "pending"}]
        });
        let resp = registry.execute_tool("TodoWrite", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================

        // //first create some
        // tool.run(json!({
        //     "todos": [{"content": "test", "status": "pending"}]
        // })).unwrap();
        // then clear

        // ================= 采用注册中心处理 start======================
        let params_json = json!({"action": "clear"});
        let resp = registry.execute_tool("TodoWrite", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================
        //let resp = tool.run(json!({"action": "clear"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        let data = resp.data;
        assert_eq!(data["stats"]["total"], 0);
    }

    #[test]
    fn test_load_todos_from_file() {
        let dir = env::temp_dir().join("todo_load_test");
        let _ = fs::remove_dir_all(&dir);
        let tool = TodoWriteTool::new(".", dir.clone());
        // Write a file manually
        let file_content = json!({
            "summary": "load test",
            "todos": [
                {"content": "loaded task", "status": "completed", "created_at": "2025-01-01T00:00:00Z", "updated_at": "2025-01-01T00:00:00Z"}
            ]
        }).to_string();
        let file_path = dir.join("test_file.json");
        fs::write(&file_path, file_content).unwrap();

        // Load
        tool.load_todos(&file_path).unwrap();

        // Run a read (e.g., retrieve current recap by doing an empty update? Just create with empty todos to see stats.)
        // Since there's no separate read action, we can just check internal state via a create with empty todos.
        // But we exposed `current_todos` via Mutex, we could lock in test and check.
        // In test, we can simply lock and check stats.
        let list = tool.current_todos.lock().unwrap();
        assert_eq!(list.summary, "load test");
        assert_eq!(list.todos.len(), 1);
        assert_eq!(list.todos[0].content, "loaded task");
    }
}