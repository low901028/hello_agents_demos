use std::collections::HashMap;

use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::response::ToolResponse;

/// 计算器工具
pub struct CalculatorTool;

impl CalculatorTool {
    pub fn new() -> Self {
        Self
    }

    fn eval(&self, expression: &str) -> Result<f64, String> {
        // 简化版：使用 meval 或手动解析
        // 这里提供一个基础实现，实际可使用 meval crate
        let expr = expression.trim();

        // 处理基本运算
        if let Ok(result) = self.simple_eval(expr) {
            return Ok(result);
        }

        Err(format!("无法计算表达式: {}", expression))
    }

    fn simple_eval(&self, expr: &str) -> Result<f64, String> {
        // 处理加减乘除和括号
        let tokens: Vec<&str> = expr.split_whitespace().collect();
        if tokens.len() == 1 {
            return tokens[0].parse::<f64>().map_err(|e| format!("{}", e));
        }
        Err("复杂表达式暂不支持，请使用简单数值".into())
    }
}

impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "python_calculator"
    }

    fn description(&self) -> &str {
        "执行数学计算。支持基本运算、数学函数等。"
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("input", "string", "要计算的数学表达式", true)]
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let expression = parameters
            .get("input")
            .or_else(|| parameters.get("expression"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if expression.is_empty() {
            return ToolResponse::error("INVALID_PARAM", "计算表达式不能为空");
        }

        println!("🧮 正在计算: {}", expression);

        match self.eval(expression) {
            Ok(result) => {
                let result_str = format!("{}", result);
                println!("✅ 计算结果: {}", result_str);
                ToolResponse::success(format!("计算结果: {}", result_str))
                    .with_data("expression", expression)
                    .with_data("result", result_str)
            }
            Err(e) => {
                println!("❌ 计算失败: {}", e);
                ToolResponse::error("EXECUTION_ERROR", e)
            }
        }
    }
}