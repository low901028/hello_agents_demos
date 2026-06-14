use std::collections::HashMap;
use serde_json::Value;
use crate::core::traits::tool::ToolParameter;

#[derive(Debug, Clone)]
pub struct ToolBase {
    pub name: String,
    pub description: String,
    pub expandable: bool,
}

impl ToolBase {
    pub fn new(name: impl Into<String>, description: impl Into<String>, expandable: bool) -> Self {
        Self { name: name.into(), description: description.into(), expandable }
    }

    pub fn validate_parameters(&self, parameters: &Value, required_params: &[String]) -> bool {
        if let Value::Object(map) = parameters {
            required_params.iter().all(|key| map.contains_key(key))
        } else { false }
    }

    pub fn to_dict(&self, params: &[ToolParameter]) -> HashMap<String, Value> {
        let mut dict = HashMap::new();
        dict.insert("name".into(), Value::String(self.name.clone()));
        dict.insert("description".into(), Value::String(self.description.clone()));
        let pv: Vec<Value> = params.iter().map(|p| serde_json::to_value(p).unwrap_or_default()).collect();
        dict.insert("parameters".into(), Value::Array(pv));
        dict
    }

    pub fn to_openai_schema(&self, params: &[ToolParameter]) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for param in params {
            let mut prop = serde_json::Map::new();
            prop.insert("type".into(), Value::String(param.param_type.clone()));
            let mut desc = param.description.clone();
            if let Some(default) = &param.default { desc = format!("{} (默认: {})", desc, default); }
            prop.insert("description".into(), Value::String(desc));
            if param.param_type == "array" {
                prop.insert("items".into(), serde_json::json!({"type": "string"}));
            }
            properties.insert(param.name.clone(), Value::Object(prop));
            if param.required { required.push(Value::String(param.name.clone())); }
        }
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": { "type": "object", "properties": properties, "required": required }
            }
        })
    }
}