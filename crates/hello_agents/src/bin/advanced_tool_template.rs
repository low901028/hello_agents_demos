// examples/advanced_tool_template.rs
// 高级工具模板 - 完整特性

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};

use hello_agents::core::traits::tool::{Tool, ToolParameter};
use hello_agents::core::types::exceptions::HelloAgentException;
use hello_agents::tools::error::ToolErrorCode;
use hello_agents::tools::response::{ToolResponse, ToolStatus};
use hello_agents::tools::tool_base::ToolBase;

pub struct AdvancedToolTemplate {
    base: ToolBase,
    api_key: Option<String>,
    max_retries: usize,
    timeout: Duration,
    enable_cache: bool,
    cache: Option<Mutex<HashMap<String, ToolResponse>>>,
    stats: Mutex<InternalStats>,
}

#[derive(Debug, Clone)]
struct InternalStats {
    total_calls: usize,
    success_calls: usize,
    error_calls: usize,
    cache_hits: usize,
}

impl Default for InternalStats {
    fn default() -> Self {
        Self { total_calls: 0, success_calls: 0, error_calls: 0, cache_hits: 0 }
    }
}

impl AdvancedToolTemplate {
    pub fn new(
        api_key: Option<String>,
        max_retries: usize,
        timeout: Duration,
        enable_cache: bool,
    ) -> Self {
        let cache = if enable_cache { Some(Mutex::new(HashMap::new())) } else { None };
        Self {
            base: ToolBase::new("advanced_tool", "高级工具模板，展示完整的工具特性", false),
            api_key,
            max_retries,
            timeout,
            enable_cache,
            cache,
            stats: Mutex::new(InternalStats::default()),
        }
    }

    fn get_cache_key(parameters: &Value) -> String {
        format!("{:x}", md5::compute(serde_json::to_string(parameters).unwrap_or_default()))
    }

    fn validate_parameters(&self, parameters: &Value) -> Option<ToolResponse> {
        let query = parameters.get("query").and_then(|v| v.as_str()).unwrap_or("");
        if query.is_empty() {
            return Some(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "参数 'query' 不能为空",
                None,
                None,
            ));
        }
        if let Some(timeout_val) = parameters.get("timeout") {
            match timeout_val.as_u64() {
                Some(t) if t > 0 => {}
                _ => return Some(ToolResponse::error(
                    ToolErrorCode::InvalidParam.as_str(),
                    "参数 'timeout' 必须大于 0",
                    None,
                    None,
                )),
            }
        }
        if let Some(format_val) = parameters.get("format").and_then(|v| v.as_str()) {
            if format_val != "json" && format_val != "text" {
                return Some(ToolResponse::error(
                    ToolErrorCode::InvalidParam.as_str(),
                    "参数 'format' 必须是 'json' 或 'text'",
                    None,
                    None,
                ));
            }
        }
        None
    }

    fn execute_logic(&self, parameters: &Value) -> Result<String, HelloAgentException> {
        let query = parameters["query"].as_str().unwrap_or("");
        std::thread::sleep(Duration::from_millis(100));
        Ok(format!("处理查询 '{}' 的结果", query))
    }

    async fn execute_logic_async(&self, parameters: Value) -> Result<String, HelloAgentException> {
        let query = parameters["query"].as_str().unwrap_or("").to_string();
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(format!("异步处理查询 '{}' 的结果", query))
    }

    pub fn get_stats(&self) -> InternalStats {
        self.stats.lock().unwrap().clone()
    }

    pub fn clear_cache(&self) {
        if let Some(cache) = &self.cache {
            cache.lock().unwrap().clear();
        }
    }
}

#[async_trait]
impl Tool for AdvancedToolTemplate {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("query", "string", "查询字符串", true, None),
            ToolParameter::new("timeout", "integer", "超时时间（秒）", false, Some(Value::Number(30.into()))),
            ToolParameter::new("format", "string", "输出格式 (json/text)", false, Some(Value::String("json".into()))),
        ]
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_calls += 1;
        }
        let start = Instant::now();

        if let Some(err) = self.validate_parameters(&parameters) {
            self.stats.lock().unwrap().error_calls += 1;
            return Ok(err);
        }

        if self.enable_cache {
            let cache_key = Self::get_cache_key(&parameters);
            if let Some(cache) = &self.cache {
                if let Some(cached) = cache.lock().unwrap().get(&cache_key) {
                    self.stats.lock().unwrap().cache_hits += 1;
                    return Ok(cached.clone());
                }
            }
        }

        let mut last_error = None;
        for attempt in 1..=self.max_retries {
            match self.execute_logic(&parameters) {
                Ok(result) => {
                    let elapsed = start.elapsed();
                    let response = ToolResponse::success(
                        format!("执行成功: {}", result),
                        Some(json!({"result": result, "parameters": parameters, "attempt": attempt})),
                        Some(json!({"time_ms": elapsed.as_millis() as u64, "retries": attempt - 1, "cache_hit": false})),
                        None,
                    );
                    if self.enable_cache {
                        if let Some(cache) = &self.cache {
                            cache.lock().unwrap().insert(Self::get_cache_key(&parameters), response.clone());
                        }
                    }
                    self.stats.lock().unwrap().success_calls += 1;
                    return Ok(response);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.max_retries {
                        std::thread::sleep(Duration::from_millis(500 * attempt as u64));
                    }
                }
            }
        }

        self.stats.lock().unwrap().error_calls += 1;
        Ok(ToolResponse::error(
            ToolErrorCode::ExecutionError.as_str(),
            &format!("执行失败: {}", last_error.unwrap()),
            None,
            Some(json!({"retries": self.max_retries})),
        ))
    }

    async fn arun(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        if let Some(err) = self.validate_parameters(&parameters) {
            return Ok(err);
        }
        let params = parameters.clone();
        match self.execute_logic_async(params).await {
            Ok(result) => Ok(ToolResponse::success(
                format!("异步执行成功: {}", result),
                Some(json!({"result": result})),
                None,
                None,
            )),
            Err(e) => Ok(ToolResponse::error(
                ToolErrorCode::ExecutionError.as_str(),
                &e.to_string(),
                None,
                None,
            )),
        }
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
            api_key: self.api_key.clone(),
            max_retries: self.max_retries,
            timeout: self.timeout,
            enable_cache: self.enable_cache,
            cache: None,
            stats: Mutex::new(self.get_stats()),
        })
    }
}

impl Drop for AdvancedToolTemplate {
    fn drop(&mut self) {
        self.clear_cache();
    }
}

// 使用示例 (main 函数)
fn main() -> Result<(), HelloAgentException> {
    // 基本使用
    let tool = AdvancedToolTemplate::new(
        Some("test_key".into()),
        3,
        Duration::from_secs(30),
        true,
    );

    let response = tool.run(json!({"query": "test query", "timeout": 10}))?;
    println!("状态: {:?}", response.status);
    println!("文本: {}", response.text);
    println!("统计: {:?}", response.stats);

    // 异步执行
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let tool = AdvancedToolTemplate::new(None, 3, Duration::from_secs(30), false);
        let response = tool.arun(json!({"query": "async test"})).await.unwrap();
        println!("异步结果: {}", response.text);
    });

    Ok(())
}