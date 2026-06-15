// examples/expandable_tool_template_async.rs
// 可展开工具模板（异步版）

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};

use hello_agents::core::traits::tool::Tool;
use hello_agents::core::traits::expandable::ExpandableTool;
use hello_agents::core::traits::tool_registry::ToolRegistry;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::core::types::response::ToolResponse;
use hello_agents::infra::tool_registry_impl::ToolRegistryImpl;

// ------------------------------------------------------------
// 可展开工具模板（父工具）
// ------------------------------------------------------------
pub struct ExpandableToolTemplate {
    /// 共享的数据存储（所有子工具共用）
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl ExpandableToolTemplate {
    pub fn new(_storage_path: Option<String>) -> Self {
        Self {
            data_store: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Tool for ExpandableToolTemplate {
    fn name(&self) -> &str { "expandable" }
    fn description(&self) -> &str { "可展开的多功能工具模板" }
    fn parameters(&self) -> Value { json!({}) }

    async fn execute(&self, _args: Value) -> Result<ToolResponse, HelloAgentError> {
        Ok(ToolResponse::error("NOT_FOUND", "此工具需要展开使用，请使用子工具"))
    }
}

impl ExpandableTool for ExpandableToolTemplate {
    fn expand(&self) -> Vec<Box<dyn Tool>> {
        vec![
            Box::new(CreateTool::new(Arc::clone(&self.data_store))),
            Box::new(ReadTool::new(Arc::clone(&self.data_store))),
            Box::new(UpdateTool::new(Arc::clone(&self.data_store))),
            Box::new(DeleteTool::new(Arc::clone(&self.data_store))),
            Box::new(ListTool::new(Arc::clone(&self.data_store))),
        ]
    }
}

// ------------------------------------------------------------
// 子工具：创建资源
// ------------------------------------------------------------
struct CreateTool {
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl CreateTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self { Self { data_store } }
}

#[async_trait]
impl Tool for CreateTool {
    fn name(&self) -> &str { "expandable_create" }
    fn description(&self) -> &str { "创建新资源" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "资源名称"},
                "content": {"type": "string", "description": "资源内容"},
                "tags": {"type": "string", "description": "标签（可选，逗号分隔）"}
            },
            "required": ["name", "content"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let tags_str = args.get("tags").and_then(|v| v.as_str()).unwrap_or("");
        let tags: Vec<String> = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

        if name.is_empty() || content.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "参数 'name' 和 'content' 不能为空"));
        }

        let mut store = self.data_store.lock().unwrap();
        if store.contains_key(&name) {
            return Ok(ToolResponse::error("CONFLICT", &format!("资源 '{}' 已存在", name)));
        }

        let resource = json!({
            "name": &name,
            "content": &content,
            "tags": tags,
            "created_at": Utc::now().to_rfc3339(),
        });
        store.insert(name.clone(), resource.clone());

        Ok(ToolResponse::success(format!("资源 '{}' 创建成功", name)))
    }
}

// ------------------------------------------------------------
// 子工具：读取资源
// ------------------------------------------------------------
struct ReadTool {
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl ReadTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self { Self { data_store } }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str { "expandable_read" }
    fn description(&self) -> &str { "读取资源内容" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "资源名称"},
                "include_metadata": {"type": "boolean", "description": "是否包含元数据"}
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let include_meta = args.get("include_metadata").and_then(|v| v.as_bool()).unwrap_or(true);

        if name.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "参数 'name' 不能为空"));
        }

        let store = self.data_store.lock().unwrap();
        match store.get(&name) {
            Some(resource) => {
                let text = if include_meta {
                    let tags: Vec<&str> = resource["tags"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                        .unwrap_or_default();
                    format!("资源 '{}':\n内容: {}\n标签: {}", name, resource["content"].as_str().unwrap_or(""), tags.join(", "))
                } else {
                    format!("资源 '{}': {}", name, resource["content"].as_str().unwrap_or(""))
                };
                Ok(ToolResponse::success(text))
            }
            None => Ok(ToolResponse::error("NOT_FOUND", &format!("资源 '{}' 不存在", name))),
        }
    }
}

// ------------------------------------------------------------
// 子工具：更新资源
// ------------------------------------------------------------
struct UpdateTool {
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl UpdateTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self { Self { data_store } }
}

#[async_trait]
impl Tool for UpdateTool {
    fn name(&self) -> &str { "expandable_update" }
    fn description(&self) -> &str { "更新资源" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "资源名称"},
                "content": {"type": "string", "description": "新内容（可选）"},
                "tags": {"type": "string", "description": "新标签（可选，逗号分隔）"}
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let new_content = args.get("content").and_then(|v| v.as_str()).map(|s| s.to_string());
        let new_tags: Option<Vec<String>> = args.get("tags").and_then(|v| v.as_str()).map(|s| {
            s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
        });

        if name.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "参数 'name' 不能为空"));
        }
        if new_content.is_none() && new_tags.is_none() {
            return Ok(ToolResponse::error("INVALID_PARAM", "至少需要提供 content 或 tags 参数"));
        }

        let mut store = self.data_store.lock().unwrap();
        match store.get_mut(&name) {
            Some(resource) => {
                let mut updated = Vec::new();
                if let Some(content) = new_content {
                    resource["content"] = Value::String(content);
                    updated.push("content");
                }
                if let Some(tags) = new_tags {
                    resource["tags"] = Value::Array(tags.into_iter().map(Value::String).collect());
                    updated.push("tags");
                }
                resource["updated_at"] = Value::String(Utc::now().to_rfc3339());
                Ok(ToolResponse::success(format!(
                    "资源 '{}' 更新成功，更新字段: {}",
                    name,
                    updated.join(", ")
                )))
            }
            None => Ok(ToolResponse::error("NOT_FOUND", &format!("资源 '{}' 不存在", name))),
        }
    }
}

// ------------------------------------------------------------
// 子工具：删除资源
// ------------------------------------------------------------
struct DeleteTool {
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl DeleteTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self { Self { data_store } }
}

#[async_trait]
impl Tool for DeleteTool {
    fn name(&self) -> &str { "expandable_delete" }
    fn description(&self) -> &str { "删除资源" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "资源名称"},
                "confirm": {"type": "boolean", "description": "确认删除（必须为 true）"}
            },
            "required": ["name", "confirm"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let confirm = args.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);

        if name.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "参数 'name' 不能为空"));
        }
        if !confirm {
            return Ok(ToolResponse::error("INVALID_PARAM", "删除操作需要确认，请设置 confirm=true"));
        }

        let mut store = self.data_store.lock().unwrap();
        match store.remove(&name) {
            Some(deleted) => Ok(ToolResponse::success(format!("资源 '{}' 已删除", name))),
            None => Ok(ToolResponse::error("NOT_FOUND", &format!("资源 '{}' 不存在", name))),
        }
    }
}

// ------------------------------------------------------------
// 子工具：列出资源
// ------------------------------------------------------------
struct ListTool {
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl ListTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self { Self { data_store } }
}

#[async_trait]
impl Tool for ListTool {
    fn name(&self) -> &str { "expandable_list" }
    fn description(&self) -> &str { "列出所有资源" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filter_tag": {"type": "string", "description": "按标签过滤（可选）"}
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let filter_tag = args.get("filter_tag").and_then(|v| v.as_str()).unwrap_or("");
        let store = self.data_store.lock().unwrap();
        let resources: Vec<Value> = if filter_tag.is_empty() {
            store.values().cloned().collect()
        } else {
            store
                .values()
                .filter(|v| {
                    v["tags"].as_array().map_or(false, |a| a.iter().any(|t| t.as_str() == Some(filter_tag)))
                })
                .cloned()
                .collect()
        };

        Ok(ToolResponse::success(format!("找到 {} 个资源", resources.len())))
    }
}

// ------------------------------------------------------------
// 使用示例（异步 main）
// ------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), HelloAgentError> {
    // 1. 创建可展开工具
    println!("=== 创建可展开工具 ===");
    let tool = ExpandableToolTemplate::new(Some("./test_data".into()));

    // 2. 查看展开后的子工具
    println!("\n=== 展开后的子工具 ===");
    let sub_tools = tool.expand();
    for st in &sub_tools {
        println!("- {}: {}", st.name(), st.description());
    }
    println!();

    // 3. 注册到 ToolRegistry（使用 expandable 注册）
    println!("=== 注册到 ToolRegistry ===");
    let mut registry = ToolRegistryImpl::new();
    registry.register_expandable(Box::new(tool));

    println!("已注册的工具:");
    for name in registry.list_tools() {
        println!("- {}", name);
    }
    println!();

    // 4. 测试子工具（异步执行）
    println!("=== 测试子工具 ===");

    // 创建
    let resp = registry.execute("expandable_create", json!({"name": "doc1", "content": "第一个文档", "tags": "重要, 文档"})).await?;
    println!("创建: {}", resp.text);

    // 读取
    let resp = registry.execute("expandable_read", json!({"name": "doc1", "include_metadata": true})).await?;
    println!("读取: {}", resp.text);

    // 更新
    let resp = registry.execute("expandable_update", json!({"name": "doc1", "content": "更新后的内容"})).await?;
    println!("更新: {}", resp.text);

    // 列出
    let resp = registry.execute("expandable_list", json!({})).await?;
    println!("列出: {}", resp.text);

    // 删除
    let resp = registry.execute("expandable_delete", json!({"name": "doc1", "confirm": true})).await?;
    println!("删除: {}", resp.text);

    Ok(())
}