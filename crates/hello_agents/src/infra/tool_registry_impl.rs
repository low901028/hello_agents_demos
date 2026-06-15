use crate::core::traits::expandable::ExpandableTool;
use crate::core::traits::tool::Tool;
use crate::core::traits::tool_registry::ToolRegistry;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::ToolResponse;
use crate::infra::circuit_breaker::CircuitBreaker;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct ToolRegistryImpl {
    pub tools: HashMap<String, Box<dyn Tool>>,
    pub breaker: Mutex<CircuitBreaker>,
}

impl ToolRegistryImpl {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            breaker: Mutex::new(CircuitBreaker::new(3, 300, true)),
        }
    }
}

#[async_trait]
impl ToolRegistry for ToolRegistryImpl {
    fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    async fn execute(&self, name: &str, args: Value) -> Result<ToolResponse, HelloAgentError> {
        // 先获取工具引用（避免跨 await 持有锁）
        let tool = {
            let mut breaker = self.breaker.lock().unwrap();
            if breaker.is_open(name) {
                return Ok(ToolResponse::error("CIRCUIT_OPEN", "工具已熔断"));
            }
            self.tools
                .get(name)
                .ok_or_else(|| HelloAgentError::General("tool not found".into()))?
                .as_ref() as &dyn Tool
            // breaker 在这里被 drop，锁释放
        };

        // 异步执行工具
        match tool.execute(args).await {
            Ok(resp) => {
                self.breaker.lock().unwrap().on_success(name);
                Ok(resp)
            }
            Err(e) => {
                self.breaker.lock().unwrap().on_failure(name);
                Err(e)
            }
        }
    }

    fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    fn register_expandable(&mut self, expandable: Box<dyn ExpandableTool>) {
        for sub in expandable.expand() {
            self.tools.insert(sub.name().to_string(), sub);
        }
    }
}
