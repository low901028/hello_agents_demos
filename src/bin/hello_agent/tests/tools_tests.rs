//! 工具系统集成测试

use hello_agents::tools::*;
use std::collections::HashMap;
use std::sync::Arc;

#[test]
fn test_calculator_tool() {
    let tool = builtin::CalculatorTool;

    let mut params = HashMap::new();
    params.insert("input".to_string(), serde_json::json!("2+3*4"));

    let response = tool.run(params);
    assert!(response.is_success());
    assert!(response.text.contains("14"));
}

#[test]
fn test_calculator_empty_expression() {
    let tool = builtin::CalculatorTool;

    let mut params = HashMap::new();
    params.insert("input".to_string(), serde_json::json!(""));

    let response = tool.run(params);
    assert!(response.is_error());
}

#[test]
fn test_calculate_function() {
    let result = builtin::calculate("10/2");
    assert!(result.contains("5"));
}

#[test]
fn test_tool_registry_register_and_execute() {
    let registry = ToolRegistry::default();

    registry.register_function("echo", "回显工具", |input| format!("ECHO: {}", input));

    let response = registry.execute_tool("echo", "hello");
    assert!(response.is_success());
    assert!(response.text.contains("ECHO: hello"));
}

#[test]
fn test_tool_registry_nonexistent_tool() {
    let registry = ToolRegistry::default();
    let response = registry.execute_tool("nonexistent", "test");
    assert!(response.is_error());
}

#[test]
fn test_tool_registry_list_tools() {
    let registry = ToolRegistry::default();
    registry.register_function("tool1", "工具1", |input| input);
    registry.register_function("tool2", "工具2", |input| input);

    let tools = registry.list_tools();
    assert!(tools.contains(&"tool1".to_string()));
    assert!(tools.contains(&"tool2".to_string()));
}

#[test]
fn test_tool_registry_unregister() {
    let registry = ToolRegistry::default();
    registry.register_function("to_remove", "待删除", |input| input);

    assert!(!registry.list_tools().is_empty());
    registry.unregister("to_remove");
    assert!(!registry.list_tools().contains(&"to_remove".to_string()));
}

#[test]
fn test_tool_registry_clear() {
    let registry = ToolRegistry::default();
    registry.register_function("tool1", "工具1", |input| input);
    registry.register_function("tool2", "工具2", |input| input);

    registry.clear();
    assert!(registry.list_tools().is_empty());
}

#[test]
fn test_tool_registry_descriptions() {
    let registry = ToolRegistry::default();
    registry.register_function("echo", "回显输入", |input| input);

    let desc = registry.get_tools_description();
    assert!(desc.contains("echo"));
    assert!(desc.contains("回显输入"));
}

#[test]
fn test_todo_list_operations() {
    let mut list = builtin::TodoList::new("测试");
    list.todos.push(builtin::TodoItem::new("任务1", "pending"));
    list.todos
        .push(builtin::TodoItem::new("任务2", "in_progress"));
    list.todos
        .push(builtin::TodoItem::new("任务3", "completed"));

    let stats = list.get_stats();
    assert_eq!(stats.get("total").copied().unwrap_or(0), 3);

    let in_progress = list.get_in_progress().unwrap();
    assert_eq!(in_progress.content, "任务2");

    let pending = list.get_pending(10);
    assert_eq!(pending.len(), 1);

    let completed = list.get_completed();
    assert_eq!(completed.len(), 1);
}

#[test]
fn test_devlog_store() {
    let mut store = builtin::DevLogStore::create("session-1", "TestAgent");
    store.append(builtin::DevLogEntry::create("decision", "选择Rust", None));
    store.append(builtin::DevLogEntry::create("progress", "完成开发", None));

    assert_eq!(store.entries.len(), 2);

    let filtered = store.filter_entries(Some("decision"), None, None);
    assert_eq!(filtered.len(), 1);

    let summary = store.generate_summary(10);
    assert!(summary.contains("2 条日志"));
}

#[test]
fn test_file_tools_read() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "Hello\nWorld\n").unwrap();

    let tool = builtin::ReadTool::new(
        dir.path().to_str().unwrap(),
        Some(dir.path().to_str().unwrap()),
        None,
    );

    let mut params = HashMap::new();
    params.insert("path".to_string(), serde_json::json!("test.txt"));

    let response = tool.run(params);
    assert!(response.is_success());
}

#[test]
fn test_file_tools_write() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let tool = builtin::WriteTool::new(
        dir.path().to_str().unwrap(),
        Some(dir.path().to_str().unwrap()),
        None,
    );

    let mut params = HashMap::new();
    params.insert("path".to_string(), serde_json::json!("output.txt"));
    params.insert("content".to_string(), serde_json::json!("Test Content"));

    let response = tool.run(params);
    assert!(response.is_success());
}

#[test]
fn test_file_tools_edit() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("edit.txt"), "Original Content\n").unwrap();

    let tool = builtin::EditTool::new(
        dir.path().to_str().unwrap(),
        Some(dir.path().to_str().unwrap()),
        None,
    );

    let mut params = HashMap::new();
    params.insert("path".to_string(), serde_json::json!("edit.txt"));
    params.insert("old_string".to_string(), serde_json::json!("Original"));
    params.insert("new_string".to_string(), serde_json::json!("Modified"));

    let response = tool.run(params);
    assert!(response.is_success());

    let content = fs::read_to_string(dir.path().join("edit.txt")).unwrap();
    assert!(content.contains("Modified"));
}
