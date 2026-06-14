use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;

use crate::core::types::exceptions::HelloAgentException;
use crate::tools::response::{ToolResponse, ToolStatus};

/// 工具参数定义
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

impl ToolParameter {
    pub fn new(
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
        required: bool,
        default: Option<Value>,
    ) -> Self {
        Self {
            name: name.into(),
            param_type: param_type.into(),
            description: description.into(),
            required,
            default,
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;

    /// 执行工具
    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException>;

    /// 获取参数定义
    fn get_parameters(&self) -> Vec<ToolParameter>;

    /// 是否可展开
    fn is_expandable(&self) -> bool {
        false
    }

    /// 展开为子工具列表
    fn expand(&self) -> Vec<Box<dyn Tool>> {
        vec![]
    }

    /// 获取展开后的子工具（钩子方法）
    fn get_tool_actions(&self) -> Vec<Box<dyn Tool>> {
        if self.is_expandable() {
            self.expand()
        } else {
            vec![]
        }
    }

    fn get_expanded_tools(&self) -> Option<Vec<Box<dyn Tool>>> {
        let tools = self.get_tool_actions();
        if tools.is_empty() {
            None
        } else {
            Some(tools)
        }
    }

    fn run_with_timing(&self, parameters: Value) -> ToolResponse {
        let start = std::time::Instant::now();
        let result = self.run(parameters.clone());
        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(mut resp) => {
                let stats = resp
                    .stats
                    .get_or_insert_with(|| Value::Object(Default::default()));
                if let Value::Object(map) = stats {
                    map.insert(
                        "time_ms".into(),
                        Value::Number(serde_json::Number::from(elapsed)),
                    );
                }
                let ctx = resp
                    .context
                    .get_or_insert_with(|| Value::Object(Default::default()));
                if let Value::Object(map) = ctx {
                    map.insert("params_input".into(), parameters);
                    map.insert(
                        "tool_name".into(),
                        Value::String(self.name().to_string()),
                    );
                }
                resp
            }
            Err(e) => ToolResponse::error(
                "INTERNAL_ERROR",
                &format!("工具执行异常: {}", e),
                Some(serde_json::json!({"time_ms": elapsed})),
                Some(serde_json::json!({
                    "params_input": parameters,
                    "tool_name": self.name()
                })),
            ),
        }
    }

    async fn arun(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        self.run(parameters)
    }

    async fn arun_with_timing(&self, parameters: Value) -> ToolResponse {
        let start = std::time::Instant::now();
        let result = self.arun(parameters.clone()).await;
        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(mut resp) => {
                let stats = resp
                    .stats
                    .get_or_insert_with(|| Value::Object(Default::default()));
                if let Value::Object(map) = stats {
                    map.insert(
                        "time_ms".into(),
                        Value::Number(serde_json::Number::from(elapsed)),
                    );
                }
                let ctx = resp
                    .context
                    .get_or_insert_with(|| Value::Object(Default::default()));
                if let Value::Object(map) = ctx {
                    map.insert("params_input".into(), parameters);
                    map.insert(
                        "tool_name".into(),
                        Value::String(self.name().to_string()),
                    );
                }
                resp
            }
            Err(e) => ToolResponse::error(
                "INTERNAL_ERROR",
                &format!("异步工具执行异常: {}", e),
                Some(serde_json::json!({"time_ms": elapsed})),
                Some(serde_json::json!({
                    "params_input": parameters,
                    "tool_name": self.name()
                })),
            ),
        }
    }

    fn box_clone(&self) -> Box<dyn Tool>;

    fn validate_parameters(&self, params: &Value) -> bool {
        let required: Vec<String> = self
            .get_parameters()
            .iter()
            .filter(|p| p.required)
            .map(|p| p.name.clone())
            .collect();
        if let Value::Object(map) = params {
            required.iter().all(|k| map.contains_key(k))
        } else {
            false
        }
    }

    fn to_dict(&self) -> HashMap<String, Value> {
        let mut dict = HashMap::new();
        dict.insert("name".into(), Value::String(self.name().to_string()));
        dict.insert(
            "description".into(),
            Value::String(self.description().to_string()),
        );
        let params: Vec<Value> = self
            .get_parameters()
            .iter()
            .map(|p| serde_json::to_value(p).unwrap_or_default())
            .collect();
        dict.insert("parameters".into(), Value::Array(params));
        dict
    }

    fn to_openai_schema(&self) -> Value {
        let parameters = self.get_parameters();
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &parameters {
            let mut prop = serde_json::Map::new();
            prop.insert(
                "type".into(),
                Value::String(param.param_type.clone()),
            );
            let mut desc = param.description.clone();
            if let Some(default) = &param.default {
                desc = format!("{} (默认: {})", desc, default);
            }
            prop.insert("description".into(), Value::String(desc));

            if param.param_type == "array" {
                prop.insert(
                    "items".into(),
                    serde_json::json!({"type": "string"}),
                );
            }
            properties.insert(param.name.clone(), Value::Object(prop));

            if param.required {
                required.push(Value::String(param.name.clone()));
            }
        }

        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name(),
                "description": self.description(),
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "required": required
                }
            }
        })
    }
}