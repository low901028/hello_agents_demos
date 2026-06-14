// examples/expandable_tool_template.rs
// 可展开工具模板

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde_json::{json, Value};

use hello_agents::core::traits::tool::{Tool, ToolParameter};
use hello_agents::core::types::exceptions::HelloAgentException;
use hello_agents::tools::error::ToolErrorCode;
use hello_agents::tools::registry::ToolRegistry;
use hello_agents::tools::response::{ToolResponse, ToolStatus};
use hello_agents::tools::tool_base::ToolBase;

// ------------------------------------------------------------
// 可展开工具模板（父工具）
// ------------------------------------------------------------
pub struct ExpandableToolTemplate {
    base: ToolBase,
    /// 共享的数据存储（所有子工具共用）
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl ExpandableToolTemplate {
    pub fn new(storage_path: Option<String>) -> Self {
        let _path = storage_path.unwrap_or_else(|| "./test_data".to_string());
        Self {
            base: ToolBase::new(
                "expandable",
                "可展开的多功能工具模板",
                true, // 可展开
            ),
            data_store: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Tool for ExpandableToolTemplate {
    fn name(&self) -> &str {
        &self.base.name
    }

    fn description(&self) -> &str {
        &self.base.description
    }
    /// 普通模式下的 run 方法：提示使用子工具
    fn run(&self, _parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        Ok(ToolResponse::error(
            ToolErrorCode::NotFound.as_str(), // 用 NotFound 代表“未实现”
            "此工具需要展开使用，请使用子工具: expandable_create, expandable_read, expandable_update, expandable_delete, expandable_list",
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![] // 父工具无参数
    }

    fn is_expandable(&self) -> bool {
        true    // 必须显式覆盖
    }

    /// 收集所有子工具
    fn get_tool_actions(&self) -> Vec<Box<dyn Tool>> {
        vec![
            Box::new(CreateTool::new(Arc::clone(&self.data_store))),
            Box::new(ReadTool::new(Arc::clone(&self.data_store))),
            Box::new(UpdateTool::new(Arc::clone(&self.data_store))),
            Box::new(DeleteTool::new(Arc::clone(&self.data_store))),
            Box::new(ListTool::new(Arc::clone(&self.data_store))),
        ]
    }

    fn get_expanded_tools(&self) -> Option<Vec<Box<dyn Tool>>> {
        let tools = self.get_tool_actions();
        if tools.is_empty() {
            None
        } else {
            println!("🔧 展开工具 '{}' -> {} 个子工具", self.name(), tools.len());
            for t in &tools {
                println!("   - {}", t.name());
            }
            Some(tools)
        }
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
            data_store: Arc::clone(&self.data_store),
        })
    }
}

// ------------------------------------------------------------
// 子工具：创建资源
// ------------------------------------------------------------
struct CreateTool {
    base: ToolBase,
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl CreateTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self {
        Self {
            base: ToolBase::new("expandable_create", "创建新资源", false),
            data_store,
        }
    }
}

impl Tool for CreateTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let name = parameters.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let content = parameters.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let tags_str = parameters.get("tags").and_then(|v| v.as_str()).unwrap_or("");
        let tags: Vec<String> = tags_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

        if name.is_empty() || content.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "参数 'name' 和 'content' 不能为空",
                None,
                None,
            ));
        }

        let mut store = self.data_store.lock().unwrap();
        if store.contains_key(&name) {
            return Ok(ToolResponse::error(
                ToolErrorCode::Conflict.as_str(),
                &format!("资源 '{}' 已存在", name),
                None,
                Some(json!({"name": name})),
            ));
        }

        let resource = json!({
            "name": &name,
            "content": &content,
            "tags": tags,
            "created_at": Utc::now().to_rfc3339(),
        });
        store.insert(name.clone(), resource.clone());

        Ok(ToolResponse::success(
            format!("资源 '{}' 创建成功", name),
            Some(json!({"resource": resource})),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("name", "string", "资源名称", true, None),
            ToolParameter::new("content", "string", "资源内容", true, None),
            ToolParameter::new("tags", "string", "标签（可选，逗号分隔）", false, Some(Value::String(String::new()))),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone(), data_store: Arc::clone(&self.data_store) })
    }
}

// ------------------------------------------------------------
// 子工具：读取资源
// ------------------------------------------------------------
struct ReadTool {
    base: ToolBase,
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl ReadTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self {
        Self { base: ToolBase::new("expandable_read", "读取资源内容", false), data_store }
    }
}

impl Tool for ReadTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let name = parameters.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let include_meta = parameters.get("include_metadata").and_then(|v| v.as_bool()).unwrap_or(true);

        if name.is_empty() {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "参数 'name' 不能为空", None, None));
        }

        let store = self.data_store.lock().unwrap();
        match store.get(&name) {
            Some(resource) => {
                let text = if include_meta {
                    let tags: Vec<&str> = resource["tags"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();
                    format!("资源 '{}':\n内容: {}\n标签: {}", name, resource["content"].as_str().unwrap_or(""), tags.join(", "))
                } else {
                    format!("资源 '{}': {}", name, resource["content"].as_str().unwrap_or(""))
                };
                Ok(ToolResponse::success(
                    text,
                    Some(if include_meta { resource.clone() } else { json!({"content": resource["content"]}) }),
                    None,
                    None,
                ))
            }
            None => Ok(ToolResponse::error(
                ToolErrorCode::NotFound.as_str(),
                &format!("资源 '{}' 不存在", name),
                None,
                Some(json!({"name": name})),
            )),
        }
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("name", "string", "资源名称", true, None),
            ToolParameter::new("include_metadata", "boolean", "是否包含元数据", false, Some(Value::Bool(true))),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone(), data_store: Arc::clone(&self.data_store) })
    }
}

// ------------------------------------------------------------
// 子工具：更新资源
// ------------------------------------------------------------
struct UpdateTool {
    base: ToolBase,
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl UpdateTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self {
        Self { base: ToolBase::new("expandable_update", "更新资源", false), data_store }
    }
}

impl Tool for UpdateTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let name = parameters.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let new_content = parameters.get("content").and_then(|v| v.as_str()).map(|s| s.to_string());
        let new_tags: Option<Vec<String>> = parameters.get("tags").and_then(|v| v.as_str()).map(|s| s.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect());

        if name.is_empty() {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "参数 'name' 不能为空", None, None));
        }
        if new_content.is_none() && new_tags.is_none() {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "至少需要提供 content 或 tags 参数", None, None));
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
                Ok(ToolResponse::success(
                    format!("资源 '{}' 更新成功，更新字段: {}", name, updated.join(", ")),
                    Some(json!({"resource": resource, "updated_fields": updated})),
                    None,
                    None,
                ))
            }
            None => Ok(ToolResponse::error(
                ToolErrorCode::NotFound.as_str(),
                &format!("资源 '{}' 不存在", name),
                None,
                Some(json!({"name": name})),
            )),
        }
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("name", "string", "资源名称", true, None),
            ToolParameter::new("content", "string", "新内容（可选）", false, None),
            ToolParameter::new("tags", "string", "新标签（可选，逗号分隔）", false, None),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone(), data_store: Arc::clone(&self.data_store) })
    }
}

// ------------------------------------------------------------
// 子工具：删除资源
// ------------------------------------------------------------
struct DeleteTool {
    base: ToolBase,
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl DeleteTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self {
        Self { base: ToolBase::new("expandable_delete", "删除资源", false), data_store }
    }
}

impl Tool for DeleteTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let name = parameters.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let confirm = parameters.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);

        if name.is_empty() {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "参数 'name' 不能为空", None, None));
        }
        if !confirm {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "删除操作需要确认，请设置 confirm=true", None, None));
        }

        let mut store = self.data_store.lock().unwrap();
        match store.remove(&name) {
            Some(deleted) => {
                Ok(ToolResponse::success(
                    format!("资源 '{}' 已删除", name),
                    Some(json!({"deleted_resource": deleted})),
                    None,
                    None,
                ))
            }
            None => Ok(ToolResponse::error(
                ToolErrorCode::NotFound.as_str(),
                &format!("资源 '{}' 不存在", name),
                None,
                Some(json!({"name": name})),
            )),
        }
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("name", "string", "资源名称", true, None),
            ToolParameter::new("confirm", "boolean", "确认删除（必须为 true）", true, Some(Value::Bool(false))),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone(), data_store: Arc::clone(&self.data_store) })
    }
}

// ------------------------------------------------------------
// 子工具：列出资源
// ------------------------------------------------------------
struct ListTool {
    base: ToolBase,
    data_store: Arc<Mutex<HashMap<String, Value>>>,
}

impl ListTool {
    fn new(data_store: Arc<Mutex<HashMap<String, Value>>>) -> Self {
        Self { base: ToolBase::new("expandable_list", "列出所有资源", false), data_store }
    }
}

impl Tool for ListTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let filter_tag = parameters.get("filter_tag").and_then(|v| v.as_str()).unwrap_or("");
        let store = self.data_store.lock().unwrap();
        let resources: Vec<Value> = if filter_tag.is_empty() {
            store.values().cloned().collect()
        } else {
            store.values().filter(|v| v["tags"].as_array().map_or(false, |a| a.iter().any(|t| t.as_str() == Some(filter_tag)))).cloned().collect()
        };

        Ok(ToolResponse::success(
            format!("找到 {} 个资源", resources.len()),
            Some(json!({
                "resources": resources,
                "count": resources.len(),
                "filter_tag": filter_tag,
            })),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("filter_tag", "string", "按标签过滤（可选）", false, Some(Value::String(String::new()))),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone(), data_store: Arc::clone(&self.data_store) })
    }
}

// ------------------------------------------------------------
// 使用示例（同步 main）
// ------------------------------------------------------------
fn main() -> Result<(), HelloAgentException> {
    // 1. 创建可展开工具
    println!("=== 创建可展开工具 ===");
    let tool = ExpandableToolTemplate::new(Some("./test_data".into()));

    // 2. 查看展开后的子工具
    println!("\n=== 展开后的子工具 ===");
    if let Some(sub_tools) = tool.get_expanded_tools() {
        for st in &sub_tools {
            println!("- {}: {}", st.name(), st.description());
        }
    }
    println!();

    // 3. 注册到 ToolRegistry（自动展开）
    println!("=== 注册到 ToolRegistry ===");
    let mut registry = ToolRegistry::new(None);
    registry.register_tool(Box::new(tool), true); // auto_expand = true

    println!("已注册的工具:");
    for name in registry.list_tools() {
        println!("- {}", name);
    }
    println!();

    // 4. 测试子工具
    println!("=== 测试子工具 ===");

    // 创建
    let resp = registry.execute_tool("expandable_create", r#"{"name": "doc1", "content": "第一个文档", "tags": "重要, 文档"}"#);
    println!("创建: {}", resp.text);

    // 读取
    let resp = registry.execute_tool("expandable_read", r#"{"name": "doc1", "include_metadata": true}"#);
    println!("读取: {}", resp.text);

    // 更新
    let resp = registry.execute_tool("expandable_update", r#"{"name": "doc1", "content": "更新后的内容"}"#);
    println!("更新: {}", resp.text);

    // 列出
    let resp = registry.execute_tool("expandable_list", r#"{}"#);
    println!("列出: {}", resp.text);

    // 删除
    let resp = registry.execute_tool("expandable_delete", r#"{"name": "doc1", "confirm": true}"#);
    println!("删除: {}", resp.text);

    Ok(())
}