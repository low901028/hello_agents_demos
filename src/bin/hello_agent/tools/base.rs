use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ToolParameter {
    pub name: String,
    pub param_type: String,
    pub description: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

impl ToolParameter {
    pub fn new(
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        ToolParameter {
            name: name.into(),
            param_type: param_type.into(),
            description: description.into(),
            required: true,
            default: None,
        }
    }
    pub fn optional(
        name: impl Into<String>,
        param_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        ToolParameter {
            name: name.into(),
            param_type: param_type.into(),
            description: description.into(),
            required: false,
            default: None,
        }
    }
    pub fn with_default(mut self, default: serde_json::Value) -> Self {
        self.default = Some(default);
        self.required = false;
        self
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn expandable(&self) -> bool {
        false
    }
    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse;
    async fn arun(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        self.run(parameters)
    }
    fn get_parameters(&self) -> Vec<ToolParameter>;

    fn run_with_timing(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        let start = Instant::now();
        let mut response = self.run(parameters.clone());
        let elapsed = start.elapsed().as_millis() as i64;
        let mut stats = response.stats.unwrap_or_default();
        stats.insert("time_ms".to_string(), serde_json::json!(elapsed));
        let mut ctx = response.context.unwrap_or_default();
        ctx.insert("tool_name".to_string(), serde_json::json!(self.name()));
        ToolResponse {
            stats: Some(stats),
            context: Some(ctx),
            ..response
        }
    }

    async fn arun_with_timing(
        &self,
        parameters: HashMap<String, serde_json::Value>,
    ) -> ToolResponse {
        let start = Instant::now();
        let mut response = self.arun(parameters.clone()).await;
        let elapsed = start.elapsed().as_millis() as i64;
        let mut stats = response.stats.unwrap_or_default();
        stats.insert("time_ms".to_string(), serde_json::json!(elapsed));
        let mut ctx = response.context.unwrap_or_default();
        ctx.insert("tool_name".to_string(), serde_json::json!(self.name()));
        ToolResponse {
            stats: Some(stats),
            context: Some(ctx),
            ..response
        }
    }

    fn get_expanded_tools(&self) -> Option<Vec<Box<dyn Tool>>> {
        None
    }

    fn to_openai_schema(&self) -> HashMap<String, serde_json::Value> {
        let params = self.get_parameters();
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for p in &params {
            let mut prop = serde_json::Map::new();
            prop.insert("type".into(), serde_json::json!(p.param_type));
            prop.insert("description".into(), serde_json::json!(&p.description));
            properties.insert(p.name.clone(), serde_json::Value::Object(prop));
            if p.required {
                required.push(p.name.clone());
            }
        }
        let mut func = serde_json::Map::new();
        func.insert("name".into(), serde_json::json!(self.name()));
        func.insert("description".into(), serde_json::json!(self.description()));
        func.insert(
            "parameters".into(),
            serde_json::json!({"type":"object","properties":properties,"required":required}),
        );
        let mut schema = HashMap::new();
        schema.insert("type".into(), serde_json::json!("function"));
        schema.insert("function".into(), serde_json::Value::Object(func));
        schema
    }
}

pub struct SimpleTool {
    name: String,
    description: String,
    parameters: Vec<ToolParameter>,
    runner: Arc<dyn Fn(HashMap<String, serde_json::Value>) -> ToolResponse + Send + Sync>,
}

impl SimpleTool {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Vec<ToolParameter>,
        runner: impl Fn(HashMap<String, serde_json::Value>) -> ToolResponse + Send + Sync + 'static,
    ) -> Self {
        SimpleTool {
            name: name.into(),
            description: description.into(),
            parameters,
            runner: Arc::new(runner),
        }
    }
}

#[async_trait]
impl Tool for SimpleTool {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        (self.runner)(parameters)
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        self.parameters.clone()
    }
}
