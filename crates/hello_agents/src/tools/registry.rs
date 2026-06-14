use std::collections::HashMap;
use std::sync::{Mutex,Arc};
use std::time::Instant;
use once_cell::sync::Lazy;
use serde_json::Value;
use crate::core::traits::tool::Tool;
use crate::tools::circuit_breaker::CircuitBreaker;
use crate::tools::error::ToolErrorCode;
use crate::tools::response::{ToolResponse, ToolStatus};

struct FunctionToolInfo { description: String, func: Arc<dyn Fn(&str) -> String + Send + Sync>, }

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    functions: HashMap<String, FunctionToolInfo>,
    pub read_metadata_cache: HashMap<String, HashMap<String, Value>>,
    pub circuit_breaker: Mutex<CircuitBreaker>,
}

impl ToolRegistry {
    pub fn new(cb: Option<CircuitBreaker>) -> Self {
        Self {
            tools: HashMap::new(),
            functions: HashMap::new(),
            read_metadata_cache: HashMap::new(),
            circuit_breaker: Mutex::new(cb.unwrap_or_else(|| CircuitBreaker::new(3, 300, true))),
        }
    }

    pub fn register_tool(&mut self, tool: Box<dyn Tool>, auto_expand: bool) {
        // 检查工具是否可展开，并提前处理展开逻辑，避免借用冲突
        if auto_expand && tool.is_expandable() {
            let expanded = tool.get_expanded_tools();
            if let Some(sub_tools) = expanded {
                for sub_tool in sub_tools {
                    let name = sub_tool.name().to_string();
                    if self.tools.contains_key(&name) {
                        println!("⚠️ 警告：工具 '{}' 已存在，将被覆盖。", name);
                    }
                    self.tools.insert(name, sub_tool);
                }
                println!(
                    "✅ 工具 '{}' 已展开为 {} 个独立工具",
                    tool.name(),
                    self.tools.len()
                );
                return; // 父工具本身不注册，直接返回
            }
        }

        // 普通工具或不展开的工具
        let tool_name = tool.name().to_string();
        if self.tools.contains_key(&tool_name) {
            println!("⚠️ 警告：工具 '{}' 已存在，将被覆盖。", tool_name);
        }

        self.tools.insert(tool_name.clone(), tool);
        println!("✅ 工具 '{}' 已注册。", tool_name);
    }

    /// 直接注册函数作为工具（简便方式）
    pub fn register_function(
        &mut self,
        func: impl Fn(&str) -> String + Send + Sync + 'static,
        name: Option<&str>,
        description: Option<&str>,
    ) {
        let name = name.unwrap_or("unknown").to_string();
        let description = description
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("执行 {}", name));

        if self.functions.contains_key(&name) {
            println!("⚠️ 警告：函数工具 '{}' 已存在，将被覆盖。", name);
        }

        self.functions.insert(
            name.clone(),
            FunctionToolInfo {
                description,
                func: Arc::new(func),
            },
        );
        println!("✅ 函数工具 '{}' 已注册。", name);
    }

    /// 注销工具（Tool 对象或函数工具）
    pub fn unregister(&mut self, name: &str) {
        if self.tools.remove(name).is_some() {
            println!("🗑️ 工具 '{}' 已注销。", name);
        } else if self.functions.remove(name).is_some() {
            println!("🗑️ 函数工具 '{}' 已注销。", name);
        } else {
            println!("⚠️ 工具 '{}' 不存在。", name);
        }
    }

    pub fn get_tool(&self, name: &str) -> Option<&dyn Tool> { self.tools.get(name).map(|b| b.as_ref()) }
    pub fn get_function(&self, name: &str) -> Option<Arc<dyn Fn(&str) -> String + Send + Sync>> { self.functions.get(name).map(|i| Arc::clone(&i.func)) }

    pub fn execute_tool(&self, name: &str, input_text: &str) -> ToolResponse {
        let mut cb = self.circuit_breaker.lock().unwrap();
        if cb.is_open(name) {
            let status = cb.get_status(name);
            let msg = format!("工具 '{}' 被禁用，{} 秒后恢复", name, status.get("recover_in_seconds").and_then(|v| v.as_u64()).unwrap_or(0));
            let resp = ToolResponse::error(ToolErrorCode::CircuitOpen.as_str(), &msg, None, Some(serde_json::json!({"tool_name": name, "circuit_status": status})));
            cb.record_result(name, &resp);
            return resp;
        }
        drop(cb);

        let resp = if let Some(tool) = self.tools.get(name) {
            let params = serde_json::from_str(input_text).unwrap_or_else(|_| {
                let mut m = serde_json::Map::new();
                m.insert("input".into(), Value::String(input_text.to_string()));
                Value::Object(m)
            });
            tool.run_with_timing(params)
        } else if let Some(info) = self.functions.get(name) {
            let func = Arc::clone(&info.func);
            let start = Instant::now();
            let result = func(input_text);
            ToolResponse::success(result.clone(), Some(serde_json::json!({"output": result})), Some(serde_json::json!({"time_ms": start.elapsed().as_millis() as u64})), Some(serde_json::json!({"tool_name": name})))
        } else {
            ToolResponse::error(ToolErrorCode::NotFound.as_str(), &format!("未找到工具 '{}'", name), None, None)
        };

        self.circuit_breaker.lock().unwrap().record_result(name, &resp);
        resp
    }

    pub fn get_tools_description(&self) -> String { /* 同前 */ unimplemented!() }
    pub fn list_tools(&self) -> Vec<String> { self.tools.keys().chain(self.functions.keys()).cloned().collect() }
    pub fn get_all_tools(&self) -> Vec<&dyn Tool> { self.tools.values().map(|b| b.as_ref()).collect() }
    pub fn clear(&mut self) { self.tools.clear(); self.functions.clear(); }
    pub fn cache_read_metadata(&mut self, path: &str, meta: HashMap<String, Value>) { self.read_metadata_cache.insert(path.to_string(), meta); }
    pub fn get_read_metadata(&self, path: &str) -> Option<&HashMap<String, Value>> { self.read_metadata_cache.get(path) }
    pub fn clear_read_cache(&mut self, path: Option<&str>) { if let Some(p) = path { self.read_metadata_cache.remove(p); } else { self.read_metadata_cache.clear(); } }
}

pub static GLOBAL_TOOL_REGISTRY: Lazy<Mutex<ToolRegistry>> = Lazy::new(|| Mutex::new(ToolRegistry::new(None)));
pub fn global_registry() -> &'static Mutex<ToolRegistry> { &GLOBAL_TOOL_REGISTRY }