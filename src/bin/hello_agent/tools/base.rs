use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;

use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;

/// 工具参数定义
#[derive(Debug, Clone)]
pub struct ToolParameter {
    pub name: String,
    pub param_type: String,
    pub description: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

impl ToolParameter {
    pub fn new(name: impl Into<String>, param_type: impl Into<String>, description: impl Into<String>, required: bool) -> Self {
        Self {
            name: name.into(),
            param_type: param_type.into(),
            description: description.into(),
            required,
            default: None,
        }
    }

    pub fn with_default(mut self, default: serde_json::Value) -> Self {
        self.default = Some(default);
        self
    }
}

/// 工具 trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具描述
    fn description(&self) -> &str;

    /// 是否可展开
    fn expandable(&self) -> bool {
        false
    }

    /// 获取参数定义
    fn get_parameters(&self) -> Vec<ToolParameter>;

    /// 执行工具
    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse;

    /// 带计时的执行
    fn run_with_timing(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let start = Instant::now();
        let mut response = self.run(parameters);
        let elapsed_ms = start.elapsed().as_millis() as u64;

        let mut stats = response.stats.unwrap_or_default();
        stats.insert("time_ms".into(), serde_json::json!(elapsed_ms));
        response.stats = Some(stats);

        let mut context = response.context.unwrap_or_default();
        context.insert("params_input".into(), serde_json::to_value(parameters).unwrap_or_default());
        context.insert("tool_name".into(), serde_json::json!(self.name()));
        response.context = Some(context);

        response
    }

    /// 异步执行
    async fn arun(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        self.run(parameters)
    }

    /// 异步带计时执行
    async fn arun_with_timing(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        self.run_with_timing(parameters)
    }

    /// 转换为 OpenAI function calling schema
    fn to_openai_schema(&self) -> serde_json::Value {
        let parameters = self.get_parameters();
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &parameters {
            properties.insert(
                param.name.clone(),
                serde_json::json!({
                    "type": param.param_type,
                    "description": param.description,
                }),
            );
            if param.required {
                required.push(param.name.clone());
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
                    "required": required,
                }
            }
        })
    }

    /// 验证参数
    fn validate_parameters(&self, parameters: &HashMap<String, serde_json::Value>) -> bool {
        self.get_parameters()
            .iter()
            .filter(|p| p.required)
            .all(|p| parameters.contains_key(&p.name))
    }
}