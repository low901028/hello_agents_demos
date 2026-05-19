use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::hello_agent::tools::base::Tool;
use crate::hello_agent::tools::circuit_breaker::CircuitBreaker;
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;

/// 工具注册表
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    functions: HashMap<String, (String, Arc<dyn Fn(String) -> String + Send + Sync>)>,
    pub read_metadata_cache: HashMap<String, HashMap<String, serde_json::Value>>,
    circuit_breaker: CircuitBreaker,
}

impl ToolRegistry {
    pub fn new(circuit_breaker: Option<CircuitBreaker>) -> Self {
        Self {
            tools: HashMap::new(),
            functions: HashMap::new(),
            read_metadata_cache: HashMap::new(),
            circuit_breaker: circuit_breaker.unwrap_or_else(|| CircuitBreaker::new(3, 300, true)),
        }
    }

    /// 注册 Tool 对象
    pub fn register_tool(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            println!("⚠️ 警告：工具 '{}' 已存在，将被覆盖。", name);
        }
        self.tools.insert(name.clone(), tool);
        println!("✅ 工具 '{}' 已注册。", name);
    }

    /// 注册函数工具
    pub fn register_function<F>(&mut self, name: impl Into<String>, description: impl Into<String>, func: F)
    where
        F: Fn(String) -> String + Send + Sync + 'static,
    {
        let name = name.into();
        if self.functions.contains_key(&name) {
            println!("⚠️ 警告：工具 '{}' 已存在，将被覆盖。", name);
        }
        self.functions.insert(name.clone(), (description.into(), Arc::new(func)));
        println!("✅ 函数工具 '{}' 已注册。", name);
    }

    /// 注销工具
    pub fn unregister(&mut self, name: &str) -> bool {
        if self.tools.remove(name).is_some() {
            println!("🗑️ 工具 '{}' 已注销。", name);
            true
        } else if self.functions.remove(name).is_some() {
            println!("🗑️ 工具 '{}' 已注销。", name);
            true
        } else {
            false
        }
    }

    /// 获取 Tool 对象
    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// 获取函数工具
    pub fn get_function(&self, name: &str) -> Option<Arc<dyn Fn(String) -> String + Send + Sync>> {
        self.functions.get(name).map(|(_, f)| f.clone())
    }

    /// 执行工具
    pub fn execute_tool(&mut self, name: &str, input_text: &str) -> ToolResponse {
        // 检查熔断器
        if self.circuit_breaker.is_open(name) {
            let status = self.circuit_breaker.get_status(name);
            return ToolResponse::error(
                ToolErrorCode::CIRCUIT_OPEN,
                format!(
                    "工具 '{}' 当前被禁用，{} 秒后可用。",
                    name,
                    status["recover_in_seconds"]
                ),
            );
        }

        let response = if let Some(tool) = self.tools.get(name) {
            let params = serde_json::from_str::<HashMap<String, serde_json::Value>>(input_text)
                .unwrap_or_else(|_| {
                    let mut map = HashMap::new();
                    map.insert("input".into(), serde_json::json!(input_text));
                    map
                });
            tool.run_with_timing(&params)
        } else if let Some((_, func)) = self.functions.get(name) {
            let result = func(input_text.to_owned());
            ToolResponse::success(&result).with_data("output", result)
        } else {
            ToolResponse::error(
                ToolErrorCode::NOT_FOUND,
                format!("未找到名为 '{}' 的工具", name),
            )
        };

        // 记录熔断器结果
        self.circuit_breaker.record_result(name, &response);

        response
    }

    /// 获取工具描述
    pub fn get_tools_description(&self) -> String {
        let mut descriptions = Vec::new();
        for tool in self.tools.values() {
            descriptions.push(format!("- {}: {}", tool.name(), tool.description()));
        }
        for (name, (desc, _)) in &self.functions {
            descriptions.push(format!("- {}: {}", name, desc));
        }
        if descriptions.is_empty() {
            "暂无可用工具".into()
        } else {
            descriptions.join("\n")
        }
    }

    /// 列出所有工具名称
    pub fn list_tools(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tools.keys().cloned().collect();
        names.extend(self.functions.keys().cloned());
        names
    }

    /// 获取所有 Tool 对象
    pub fn get_all_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    /// 清空
    pub fn clear(&mut self) {
        self.tools.clear();
        self.functions.clear();
        println!("🧹 所有工具已清空。");
    }
}