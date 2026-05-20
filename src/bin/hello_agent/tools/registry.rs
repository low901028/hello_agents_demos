use crate::hello_agent::tools::base::Tool;
use crate::hello_agent::tools::circuit_breaker::CircuitBreaker;
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::{ToolResponse, ToolStatus};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
    functions: RwLock<HashMap<String, FunctionTool>>,
    read_metadata_cache: RwLock<HashMap<String, HashMap<String, serde_json::Value>>>,
    circuit_breaker: RwLock<CircuitBreaker>,
}

#[derive(Clone)]
struct FunctionTool {
    description: String,
    func: Arc<dyn Fn(String) -> String + Send + Sync>,
}

impl ToolRegistry {
    pub fn new(cb: Option<CircuitBreaker>) -> Self {
        ToolRegistry {
            tools: RwLock::new(HashMap::new()),
            functions: RwLock::new(HashMap::new()),
            read_metadata_cache: RwLock::new(HashMap::new()),
            circuit_breaker: RwLock::new(cb.unwrap_or_default()),
        }
    }

    pub fn register_tool(&self, tool: Arc<dyn Tool>, auto_expand: bool) {
        if auto_expand && tool.expandable() {
            if let Some(expanded) = tool.get_expanded_tools() {
                let mut tools = self.tools.write();
                for t in expanded {
                    tools.insert(t.name().to_string(), t.into());
                }
                return;
            }
        }
        self.tools.write().insert(tool.name().to_string(), tool);
    }

    pub fn register_function<F: Fn(String) -> String + Send + Sync + 'static>(
        &self,
        name: &str,
        description: &str,
        func: F,
    ) {
        self.functions.write().insert(
            name.to_string(),
            FunctionTool {
                description: description.to_string(),
                func: Arc::new(func),
            },
        );
    }

    pub fn unregister(&self, name: &str) {
        self.tools.write().remove(name);
        self.functions.write().remove(name);
    }

    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().get(name).cloned()
    }

    pub fn execute_tool(&self, name: &str, input_text: &str) -> ToolResponse {
        {
            let mut cb = self.circuit_breaker.write();
            if cb.is_open(name) {
                let status = cb.get_status(name);
                return ToolResponse::error(
                    ToolErrorCode::CircuitOpen.as_str(),
                    &format!(
                        "工具'{}'被禁用，{}秒后可用",
                        name, status.recover_in_seconds
                    ),
                );
            }
        }

        let response = {
            let tools = self.tools.read();
            if let Some(tool) = tools.get(name) {
                Some(tool.run_with_timing(Self::parse_params(input_text)))
            } else {
                None
            }
        };

        if let Some(resp) = response {
            self.circuit_breaker.write().record_result(name, &resp);
            return resp;
        }

        if let Some(ft) = self.functions.read().get(name).cloned() {
            let start = Instant::now();
            let result = (ft.func)(input_text.to_string());
            let elapsed = start.elapsed().as_millis() as i64;
            let mut data = HashMap::new();
            data.insert("output".into(), serde_json::json!(&result));
            let mut stats = HashMap::new();
            stats.insert("time_ms".into(), serde_json::json!(elapsed));
            let resp = ToolResponse::success(result, data).with_stats(stats);
            self.circuit_breaker.write().record_result(name, &resp);
            return resp;
        }

        ToolResponse::error(
            ToolErrorCode::NotFound.as_str(),
            &format!("未找到工具'{}'", name),
        )
    }

    fn parse_params(input: &str) -> HashMap<String, serde_json::Value> {
        serde_json::from_str(input).unwrap_or_else(|_| {
            let mut m = HashMap::with_capacity(1);
            m.insert("input".into(), serde_json::json!(input));
            m
        })
    }

    pub fn list_tools(&self) -> Vec<String> {
        let mut names: Vec<String> = self.tools.read().keys().cloned().collect();
        names.extend(self.functions.read().keys().cloned());
        names
    }

    pub fn get_all_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.read().values().cloned().collect()
    }
    pub fn clear(&self) {
        self.tools.write().clear();
        self.functions.write().clear();
    }
    pub fn cache_read_metadata(&self, path: &str, meta: HashMap<String, serde_json::Value>) {
        self.read_metadata_cache
            .write()
            .insert(path.to_string(), meta);
    }
    pub fn get_read_metadata(&self, path: &str) -> Option<HashMap<String, serde_json::Value>> {
        self.read_metadata_cache.read().get(path).cloned()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        ToolRegistry::new(None)
    }
}

use std::sync::OnceLock;
static GLOBAL_REGISTRY: OnceLock<ToolRegistry> = OnceLock::new();
pub fn global_registry() -> &'static ToolRegistry {
    GLOBAL_REGISTRY.get_or_init(|| ToolRegistry::new(None))
}
