use evalexpr::{ContextWithMutableVariables, HashMapContext};
use serde_json::Value;
use crate::core::traits::tool::{Tool, ToolParameter};
use crate::core::types::exceptions::HelloAgentException;
use crate::tools::error::ToolErrorCode;
use crate::tools::response::{ToolResponse, ToolStatus};
use crate::tools::tool_base::ToolBase;

pub struct CalculatorTool { base: ToolBase }

impl CalculatorTool {
    pub fn new() -> Self {
        Self { base: ToolBase::new("python_calculator", "执行数学计算。支持基本运算、数学函数等。例如：2+3*4, sqrt(16), sin(pi/2)等。", false) }
    }
}

impl Tool for CalculatorTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("input", "string", "要计算的数学表达式，支持基本运算和数学函数", true, None)]
    }
    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let expr = parameters.get("input").or_else(|| parameters.get("expression")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        if expr.is_empty() { return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "计算表达式不能为空", None, None)); }
        let preprocessed = expr.replace("**", "^");
        let mut ctx: HashMapContext = evalexpr::HashMapContext::new();
        let _ = ctx.set_value("pi".into(), evalexpr::Value::Float(std::f64::consts::PI));
        let _ = ctx.set_value("e".into(), evalexpr::Value::Float(std::f64::consts::E));
        match evalexpr::eval_with_context(&preprocessed, &ctx) {
            Ok(result) => {
                let num = result.as_float().unwrap_or(0.0);
                Ok(ToolResponse::success(format!("计算结果: {}", num), Some(serde_json::json!({"expression": expr, "result": num})), None, None))
            }
            Err(e) => {
                let code = if format!("{}", e).contains("invalid") { ToolErrorCode::InvalidFormat.as_str() } else { ToolErrorCode::ExecutionError.as_str() };
                Ok(ToolResponse::error(code, &format!("计算失败: {}", e), None, Some(serde_json::json!({"expression": expr}))))
            }
        }
    }
    fn box_clone(&self) -> Box<dyn Tool> { Box::new(Self { base: self.base.clone() }) }
}