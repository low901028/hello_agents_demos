//! tool_registry.rs
//! 工具注册表 - HelloAgents 原生工具系统

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use once_cell::sync::Lazy;
use serde_json::Value;
use tokio::sync::Mutex;
use crate::tools::circuit_breaker::CircuitBreaker;
use crate::tools::tool_base::{Tool};
use crate::tools::tool_error::ToolErrorCode;
use crate::tools::tool_response::{ToolResponse};

/// 解析输入参数：尝试 JSON 解析，失败则包装为 `{"input": input_text}`
fn parse_parameters(input: &str) -> Value {
    serde_json::from_str(input).unwrap_or_else(|_| {
        let mut map = serde_json::Map::new();
        map.insert("input".to_string(), Value::String(input.to_string()));
        Value::Object(map)
    })
}

// ------------------------------------------------------------
// 函数工具包装信息
// ------------------------------------------------------------
struct FunctionToolInfo {
    description: String,
    func: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

// ------------------------------------------------------------
// ToolRegistry 结构体及实现
// ------------------------------------------------------------

/// HelloAgents 工具注册表
///
/// 提供工具的注册、管理和执行功能。
/// 支持两种工具注册方式：
/// 1. Tool 对象注册（推荐）
/// 2. 函数直接注册（简便）
pub struct ToolRegistry {
    /// 工具对象注册表
    tools: HashMap<String, Box<dyn Tool>>,
    /// 函数工具注册表
    functions: HashMap<String, FunctionToolInfo>,
    /// 文件元数据缓存（用于乐观锁机制）
    pub read_metadata_cache: HashMap<String, HashMap<String, Value>>,
    /// 熔断器（默认启用）
    pub circuit_breaker: CircuitBreaker,
}

impl ToolRegistry {
    /// 创建新的工具注册表
    ///
    /// # Arguments
    /// * `circuit_breaker` - 可选的熔断器，若未提供则使用默认配置
    pub fn new(circuit_breaker: Option<CircuitBreaker>) -> Self {
        Self {
            tools: HashMap::new(),
            functions: HashMap::new(),
            read_metadata_cache: HashMap::new(),
            circuit_breaker: circuit_breaker
                .unwrap_or_else(|| CircuitBreaker::new(3, 300, true)),
        }
    }

    /// 注册 Tool 对象
    ///
    /// # Arguments
    /// * `tool` - Tool 实例
    /// * `auto_expand` - 是否自动展开可展开的工具（默认 True）
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
    ///
    /// 支持自动从函数信息提取名称和描述。
    /// 注意：Rust 中无法自动提取函数名和 docstring，需手动提供 name 和 description。
    ///
    /// # Arguments
    /// * `func` - 工具函数，接收输入字符串，返回结果字符串
    /// * `name` - 工具名称（可选，默认为 "unknown"）
    /// * `description` - 工具描述（可选，默认为 "执行 {name}"）
    ///
    /// # Example
    /// ```rust
    /// registry.register_function(|input| format!("处理: {}", input), Some("my_tool"), Some("这是我的工具"));
    /// ```
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

    /// 注销工具
    ///
    /// # Arguments
    /// * `name` - 工具名称
    pub fn unregister(&mut self, name: &str) {
        if self.tools.remove(name).is_some() {
            println!("🗑️ 工具 '{}' 已注销。", name);
        } else if self.functions.remove(name).is_some() {
            println!("🗑️ 函数工具 '{}' 已注销。", name);
        } else {
            println!("⚠️ 工具 '{}' 不存在。", name);
        }
    }

    /// 获取 Tool 对象
    pub fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|b| b.as_ref())
    }

    /// 获取工具函数
    pub fn get_function(&self, name: &str) -> Option<Arc<dyn Fn(&str) -> String + Send + Sync>> {
        self.functions.get(name).map(|info| Arc::clone(&info.func))
    }

    /// 执行工具，返回 ToolResponse 对象（带熔断器保护）
    ///
    /// # Arguments
    /// * `name` - 工具名称
    /// * `input_text` - 输入参数（JSON 字符串或普通文本）
    ///
    /// # Returns
    /// ToolResponse: 标准化的工具响应对象
    pub fn execute_tool(&mut self, name: &str, input_text: &str) -> ToolResponse {
        // 检查熔断器
        if self.circuit_breaker.is_open(name) {
            let status = self.circuit_breaker.get_status(name);
            let recover_in = status
                .get("recover_in_seconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let msg = format!(
                "工具 '{}' 当前被禁用，由于连续失败。{} 秒后可用。",
                name, recover_in
            );
            let resp = ToolResponse::error(
                ToolErrorCode::CircuitOpen.as_str(),
                &msg,
                None,
                Some(serde_json::json!({
                    "tool_name": name,
                    "circuit_status": status
                })),
            );
            self.circuit_breaker.record_result(name, &resp);
            return resp;
        }

        let response;

        // 优先查找 Tool 对象
        if let Some(tool) = self.tools.get(name) {
            // 解析参数（支持 JSON 字符串或纯文本）
            let parameters = parse_parameters(input_text);
            response = tool.run_with_timing(parameters);
        }
        // 查找函数工具
        else if let Some(info) = self.functions.get(name) {
            let func = Arc::clone(&info.func);
            let start = Instant::now();
            let result = func(input_text);
            let elapsed_ms = start.elapsed().as_millis() as u64;

            response = ToolResponse::success(
                result.clone(),
                Some(serde_json::json!({"output": result})),
                Some(serde_json::json!({"time_ms": elapsed_ms})),
                Some(serde_json::json!({"tool_name": name, "input": input_text})),
            );
        }
        // 工具不存在
        else {
            response = ToolResponse::error(
                ToolErrorCode::NotFound.as_str(),
                &format!("未找到名为 '{}' 的工具", name),
                None,
                Some(serde_json::json!({"tool_name": name})),
            );
        }

        // 记录熔断器结果
        self.circuit_breaker.record_result(name, &response);
        response
    }

    /// 获取所有可用工具的格式化描述字符串
    ///
    /// # Returns
    /// 工具描述字符串，用于构建提示词
    pub fn get_tools_description(&self) -> String {
        let mut descriptions = Vec::new();

        // Tool 对象描述
        for tool in self.tools.values() {
            descriptions.push(format!("- {}: {}", tool.name(), tool.base().description));
        }

        // 函数工具描述
        for (name, info) in &self.functions {
            descriptions.push(format!("- {}: {}", name, info.description));
        }

        if descriptions.is_empty() {
            "暂无可用工具".to_string()
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
    pub fn get_all_tools(&self) -> Vec<&dyn Tool> {
        self.tools.values().map(|b| b.as_ref()).collect()
    }

    /// 清空所有工具
    pub fn clear(&mut self) {
        self.tools.clear();
        self.functions.clear();
        println!("🧹 所有工具已清空。");
    }

    // ==================== 乐观锁机制支持 ====================

    /// 缓存 Read 工具获取的文件元数据
    /// Args:
    ///    file_path: 文件路径（相对于 project_root）
    ///    metadata: 文件元数据字典，包含：
    ///       - file_mtime_ms: 文件修改时间（毫秒时间戳）
    ///       - file_size_bytes: 文件大小（字节）
    pub fn cache_read_metadata(&mut self, file_path: &str, metadata: HashMap<String, Value>) {
        self.read_metadata_cache
            .insert(file_path.to_string(), metadata);
    }

    /// 获取缓存的文件元数据
    /// Args:
    ///    file_path: 文件路径
    ///
    /// Returns:
    ///    文件元数据字典，如果不存在则返回 None
    pub fn get_read_metadata(&self, file_path: &str) -> Option<&HashMap<String, Value>> {
        self.read_metadata_cache.get(file_path)
    }

    /// 清空文件元数据缓存
    pub fn clear_read_cache(&mut self, file_path: Option<&str>) {
        if let Some(path) = file_path {
            self.read_metadata_cache.remove(path);
        } else {
            self.read_metadata_cache.clear();
        }
    }
}


// ============================================================
// 全局唯一工具注册表
// ============================================================
/// 全局工具注册表实例（线程安全，延迟初始化）
pub static GLOBAL_TOOL_REGISTRY: Lazy<Mutex<ToolRegistry>> =
    Lazy::new(|| Mutex::new(ToolRegistry::new(None)));

/// 获取全局工具注册表的引用
pub fn global_registry() -> &'static Mutex<ToolRegistry> {
    &GLOBAL_TOOL_REGISTRY
}

// ============================================================
// 测试用例（可置于文件末尾或独立测试文件）
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::circuit_breaker::CircuitBreaker;
    use crate::tools::tool_base::{ToolBase, ToolParameter};
    use crate::tools::tool_response::ToolStatus;
    use serde_json::json;
    use crate::core::exceptions::HelloAgentException;

    // 定义一个简单的 EchoTool
    struct EchoTool {
        base: ToolBase,
        parameters: Vec<ToolParameter>,
    }

    impl EchoTool {
        fn new() -> Self {
            Self {
                base: ToolBase::new("echo", "回显输入", false),
                parameters: vec![ToolParameter::new(
                    "input",
                    "string",
                    "需要回显的文本",
                    true,
                    None,
                )],
            }
        }
    }

    impl Tool for EchoTool {
        fn name(&self) -> &str { &self.base.name }
        fn base(&self) -> &ToolBase { &self.base }
        fn base_mut(&mut self) -> &mut ToolBase { &mut self.base }
        fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
            let input = parameters["input"].as_str().unwrap_or("");
            Ok(ToolResponse::success(input, None, None, None))
        }
        fn get_parameters(&self) -> Vec<ToolParameter> {
            self.parameters.clone()
        }
        fn box_clone(&self) -> Box<dyn Tool> {
            Box::new(Self {
                base: self.base.clone(),
                parameters: self.parameters.clone(),
            })
        }
    }

    #[test]
    fn test_register_and_list() {
        let mut reg = ToolRegistry::new(None);
        reg.register_tool(Box::new(EchoTool::new()), false);
        assert_eq!(reg.list_tools(), vec!["echo"]);
    }

    #[test]
    fn test_register_function_and_execute() {
        let mut reg = ToolRegistry::new(None);
        reg.register_function(|input| format!("hello {}", input), Some("greet"), Some("打招呼"));
        let resp = reg.execute_tool("greet", "world");
        assert_eq!(resp.status, ToolStatus::Success);
        assert_eq!(resp.text, "hello world");
    }

    #[test]
    fn test_execute_tool_object() {
        let mut reg = ToolRegistry::new(None);
        reg.register_tool(Box::new(EchoTool::new()), false);
        let resp = reg.execute_tool("echo", "test");
        assert_eq!(resp.status, ToolStatus::Success);
        assert_eq!(resp.text, "test");
    }

    #[test]
    fn test_execute_tool_not_found() {
        let mut reg = ToolRegistry::new(None);
        let resp = reg.execute_tool("nonexistent", "x");
        assert_eq!(resp.status, ToolStatus::Error);
        assert_eq!(resp.error_info.as_ref().unwrap().code, ToolErrorCode::NotFound.as_str());
    }

    #[test]
    fn test_circuit_breaker_trigger() {
        let cb = CircuitBreaker::new(1, 60, true);
        let mut reg = ToolRegistry::new(Some(cb));
        struct FailTool { base: ToolBase }
        impl FailTool {
            fn new() -> Self { Self { base: ToolBase::new("fail", "总是失败", false) } }
        }
        impl Tool for FailTool {
            fn name(&self) -> &str { &self.base.name }
            fn base(&self) -> &ToolBase { &self.base }
            fn base_mut(&mut self) -> &mut ToolBase { &mut self.base }
            fn run(&self, _: Value) -> Result<ToolResponse, HelloAgentException> {
                Err(HelloAgentException::ToolException("fail".into()))
            }
            fn get_parameters(&self) -> Vec<ToolParameter> { vec![] }
            fn box_clone(&self) -> Box<dyn Tool> {
                Box::new(Self { base: self.base.clone() })
            }
        }
        reg.register_tool(Box::new(FailTool::new()), false);

        let resp1 = reg.execute_tool("fail", "");
        assert_eq!(resp1.status, ToolStatus::Error);
        assert!(reg.circuit_breaker.is_open("fail"));
        let resp2 = reg.execute_tool("fail", "");
        assert_eq!(resp2.status, ToolStatus::Error);
        assert_eq!(resp2.error_info.as_ref().unwrap().code, ToolErrorCode::CircuitOpen.as_str());
    }

    #[test]
    fn test_get_tools_description() {
        let mut reg = ToolRegistry::new(None);
        reg.register_tool(Box::new(EchoTool::new()), false);
        reg.register_function(|_| "".into(), Some("myfn"), Some("my desc"));
        let desc = reg.get_tools_description();
        assert!(desc.contains("echo"));
        assert!(desc.contains("myfn"));
    }

    #[test]
    fn test_clear() {
        let mut reg = ToolRegistry::new(None);
        reg.register_tool(Box::new(EchoTool::new()), false);
        reg.clear();
        assert!(reg.list_tools().is_empty());
    }

    #[test]
    fn test_metadata_cache() {
        let mut reg = ToolRegistry::new(None);
        let mut meta = HashMap::new();
        meta.insert("size".to_string(), json!(1024));
        reg.cache_read_metadata("/test", meta);
        let cached = reg.get_read_metadata("/test").unwrap();
        assert_eq!(cached["size"], 1024);
        reg.clear_read_cache(Some("/test"));
        assert!(reg.get_read_metadata("/test").is_none());
    }

    #[test]
    fn test_global_registry() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            // 全局注册中心
            let guard = global_registry();
            let mut registry = guard.lock().await;
            // ========================== 注册tool ========================= //
            // 注册tool
            registry.register_tool(Box::new(EchoTool::new()), true);
            // 在注册完成后，执行tool
            let resp = registry.execute_tool("echo", "global test");

            assert_eq!(resp.status, ToolStatus::Success);
            assert_eq!(resp.text, "global test");

            // ========================== 注册function ========================= //
            fn my_tool(input: &str) -> String {
                format!("处理: {input}")
            }
            // 注册
            registry.register_function(my_tool, Some("custom_name"), Some("自定义描述"));
            let resp = registry.execute_tool("custom_name", "这是我的工具");
            assert_eq!(resp.status, ToolStatus::Success);
            assert_eq!(resp.text, "处理: 这是我的工具");
            // 清理
            registry.clear();
        });
    }
}