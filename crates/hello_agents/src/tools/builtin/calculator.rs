use crate::core::traits::tool::Tool;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::ToolResponse;
use async_trait::async_trait;
use evalexpr::{ContextWithMutableVariables, HashMapContext};
use serde_json::Value;

pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }
    fn description(&self) -> &str {
        "执行数学计算。支持基本运算、数学函数等。例如：2+3*4, sqrt(16), sin(pi/2)等。"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "要计算的数学表达式"
                }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let expr = args["expression"].as_str().unwrap_or("");
        if expr.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "计算表达式不能为空"));
        }

        let preprocessed = expr.replace("**", "^");
        let mut ctx: HashMapContext = evalexpr::HashMapContext::new();
        let _ = ctx.set_value("pi".into(), evalexpr::Value::Float(std::f64::consts::PI));
        let _ = ctx.set_value("e".into(), evalexpr::Value::Float(std::f64::consts::E));

        match evalexpr::eval_with_context(&preprocessed, &ctx) {
            Ok(result) => {
                let num = result.as_float().unwrap_or(0.0);
                Ok(ToolResponse::success(format!("计算结果: {}", num)))
            }
            Err(e) => {
                let code = if format!("{}", e).contains("invalid") {
                    "INVALID_FORMAT"
                } else {
                    "EXECUTION_ERROR"
                };
                Ok(ToolResponse::error(code, &format!("计算失败: {}", e)))
            }
        }
    }
}
