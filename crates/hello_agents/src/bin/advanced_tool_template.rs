// examples/advanced_tool_template.rs
// 高级工具模板 - 适配新架构

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use hello_agents::core::traits::tool::Tool;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::tools::error::ToolErrorCode;
use serde_json::{json, Value};
use hello_agents::core::types::response::ToolResponse;

pub struct AdvancedToolTemplate {
    name: String,
    description: String,
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
        Self {
            total_calls: 0,
            success_calls: 0,
            error_calls: 0,
            cache_hits: 0,
        }
    }
}

impl AdvancedToolTemplate {
    pub fn new(
        api_key: Option<String>,
        max_retries: usize,
        timeout: Duration,
        enable_cache: bool,
    ) -> Self {
        let cache = if enable_cache {
            Some(Mutex::new(HashMap::new()))
        } else {
            None
        };
        Self {
            name: "advanced_tool".into(),
            description: "高级工具模板，展示完整的工具特性".into(),
            api_key,
            max_retries,
            timeout,
            enable_cache,
            cache,
            stats: Mutex::new(InternalStats::default()),
        }
    }

    fn get_cache_key(parameters: &Value) -> String {
        format!(
            "{:x}",
            md5::compute(serde_json::to_string(parameters).unwrap_or_default())
        )
    }

    fn validate_parameters(&self, parameters: &Value) -> Option<ToolResponse> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if query.is_empty() {
            return Some(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "参数 'query' 不能为空",
            ));
        }
        if let Some(timeout_val) = parameters.get("timeout") {
            if !timeout_val.is_u64() || timeout_val.as_u64().unwrap() == 0 {
                return Some(ToolResponse::error(
                    ToolErrorCode::InvalidParam.as_str(),
                    "参数 'timeout' 必须大于 0",
                ));
            }
        }
        if let Some(format_val) = parameters.get("format").and_then(|v| v.as_str()) {
            if format_val != "json" && format_val != "text" {
                return Some(ToolResponse::error(
                    ToolErrorCode::InvalidParam.as_str(),
                    "参数 'format' 必须是 'json' 或 'text'",
                ));
            }
        }
        None
    }

    // 实际的业务逻辑（同步，但会在异步上下文中通过 spawn_blocking 运行以避免阻塞）
    fn execute_logic_sync(&self, parameters: &Value) -> Result<String, HelloAgentError> {
        let query = parameters["query"].as_str().unwrap_or("");
        // 模拟耗时操作
        std::thread::sleep(Duration::from_millis(100));
        Ok(format!("处理查询 '{}' 的结果", query))
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
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "查询字符串" },
                "timeout": { "type": "integer", "description": "超时时间（秒）", "default": 30 },
                "format": { "type": "string", "description": "输出格式 (json/text)", "default": "json" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, parameters: Value) -> Result<ToolResponse, HelloAgentError> {
        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_calls += 1;
        }
        let start = Instant::now();

        // 1. 参数验证
        if let Some(err) = self.validate_parameters(&parameters) {
            self.stats.lock().unwrap().error_calls += 1;
            return Ok(err);
        }

        // 2. 检查缓存
        if self.enable_cache {
            let cache_key = Self::get_cache_key(&parameters);
            if let Some(cache) = &self.cache {
                let cache_map = cache.lock().unwrap();
                if let Some(cached) = cache_map.get(&cache_key) {
                    self.stats.lock().unwrap().cache_hits += 1;
                    return Ok(cached.clone());
                }
            }
        }

        // 3. 异步重试执行（将同步逻辑放在 spawn_blocking 中以避免阻塞异步运行时）
        let max_retries = self.max_retries;
        let timeout = self.timeout;
        let enable_cache = self.enable_cache;
        let cache = self.cache.as_ref();
        let stats = &self.stats;
        let parameters_clone = parameters.clone();

        let result = tokio::task::spawn_blocking(move || {
            let mut last_error = None;
            for attempt in 1..=max_retries {
                match AdvancedToolTemplate::execute_logic_sync(
                    &AdvancedToolTemplate {
                        name: String::new(),
                        description: String::new(),
                        api_key: None,
                        max_retries,
                        timeout,
                        enable_cache,
                        cache: None,
                        stats: Mutex::new(InternalStats::default()),
                    },
                    &parameters_clone,
                ) {
                    Ok(res) => return Ok(res),
                    Err(e) => {
                        last_error = Some(e);
                        if attempt < max_retries {
                            std::thread::sleep(Duration::from_millis(500 * attempt as u64));
                        }
                    }
                }
            }
            Err(last_error.unwrap())
        })
        .await
        .map_err(|e| HelloAgentError::General(format!("spawn_blocking error: {}", e)))?;

        match result {
            Ok(res) => {
                let elapsed = start.elapsed();
                let response = ToolResponse::success_with(
                    format!("执行成功: {}", res),
                    Some(json!({"result": res, "parameters": parameters, "attempt": 1})), // attempt 可后续从上下文获取
                    Some(
                        json!({"time_ms": elapsed.as_millis() as u64, "retries": 0, "cache_hit": false}),
                    ),
                    None,
                );
                // 缓存结果
                if let Some(cache) = cache {
                    cache
                        .lock()
                        .unwrap()
                        .insert(Self::get_cache_key(&parameters), response.clone());
                }
                stats.lock().unwrap().success_calls += 1;
                Ok(response)
            }
            Err(e) => {
                stats.lock().unwrap().error_calls += 1;
                Ok(ToolResponse::error(
                    ToolErrorCode::ExecutionError.as_str(),
                    &format!("执行失败: {}", e),
                ))
            }
        }
    }
}

impl Drop for AdvancedToolTemplate {
    fn drop(&mut self) {
        self.clear_cache();
    }
}

// 使用示例 (main 函数)
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 基本使用
    let tool = AdvancedToolTemplate::new(Some("test_key".into()), 3, Duration::from_secs(30), true);

    let response = tool
        .execute(json!({"query": "test query", "timeout": 10}))
        .await?;
    println!("状态: {:?}", response.status);
    println!("文本: {}", response.text);
    println!("统计: {:?}", response.stats);

    // 异步执行（execute 本身就是异步）
    let tool2 = AdvancedToolTemplate::new(None, 3, Duration::from_secs(30), false);
    let response2 = tool2.execute(json!({"query": "async test"})).await?;
    println!("异步结果: {}", response2.text);

    Ok(())
}
