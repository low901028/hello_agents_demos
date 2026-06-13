//! calculator_tool.rs
//! 计算器工具 - 对应 Python CalculatorTool
//! TODO： 后续会基于evalexpr 重新实现
use serde_json::Value;

use crate::core::exceptions::HelloAgentException;
use crate::tools::tool_base::{Tool, ToolBase, ToolParameter};
use crate::tools::tool_error::ToolErrorCode;
use crate::tools::tool_response::{ToolResponse, ToolStatus};

///
/// 支持基本数学运算、数学函数和常用常量。
/// 内部使用 `meval` 库进行表达式求值。
///
/// # 注意事项
/// - Python 使用 `**` 作为幂运算符，而本工具在求值前会自动将 `**` 替换为 `^`，以适配 `meval` 的语法。
/// - 支持的函数：`abs`, `round`, `max`, `min`, `sum`, `sqrt`, `sin`, `cos`, `tan`, `log`, `exp`
/// - 支持的常量：`pi`, `e`
pub struct CalculatorTool {
    base: ToolBase,
}

impl CalculatorTool {
    pub fn new() -> Self {
        Self {
            base: ToolBase::new(
                "python_calculator",
                "执行数学计算。支持基本运算、数学函数等。例如：2+3*4, sqrt(16), sin(pi/2)等。",
                false,
            ),
        }
    }

    /// 构造表达式上下文（函数 + 常量）
    fn create_context() -> meval::Context<'static> {
        let mut ctx = meval::Context::new();

        // 注册数学函数（签名均为 fn(&[f64]) -> f64）
        ctx.funcn("abs", |args: &[f64]| args[0].abs(), ..);
        ctx.funcn("round", |args: &[f64]| args[0].round(), ..);
        ctx.funcn("max", |args: &[f64]| {
            args.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        }, ..);
        ctx.funcn("min", |args: &[f64]| {
            args.iter().copied().fold(f64::INFINITY, f64::min)
        }, ..);
        ctx.funcn("sum", |args: &[f64]| args.iter().sum(), ..);
        ctx.funcn("sqrt", |args: &[f64]| args[0].sqrt(), ..);
        ctx.funcn("sin", |args: &[f64]| args[0].sin(), ..);
        ctx.funcn("cos", |args: &[f64]| args[0].cos(), ..);
        ctx.funcn("tan", |args: &[f64]| args[0].tan(), ..);
        ctx.funcn("log", |args: &[f64]| args[0].ln(),..); // Python math.log 为自然对数
        ctx.funcn("exp", |args:&[f64]| args[0].exp(), ..);

        // 注册常量
        ctx.var("pi", std::f64::consts::PI);
        ctx.var("e", std::f64::consts::E);

        ctx
    }

    /// 预处理表达式：将 Python 风格的 `**` 替换为 `^`
    fn preprocess(expr: &str) -> String {
        expr.replace("**", "^")
    }
}

impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        &self.base.name
    }

    fn base(&self) -> &ToolBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut ToolBase {
        &mut self.base
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        // 支持 input 和 expression 两种参数名
        let expression = parameters
            .get("input")
            .or_else(|| parameters.get("expression"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if expression.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "计算表达式不能为空",
                None,
                None,
            ));
        }

        println!("🧮 正在计算: {}", expression);

        // 预处理表达式，使 Python 语法兼容 meval
        let preprocessed = Self::preprocess(&expression);

        // 创建上下文（函数/常量）并求值
        let ctx = Self::create_context();
        match meval::eval_str_with_context(&preprocessed, &ctx) {
            Ok(result) => {
                let result_str = result.to_string();
                println!("✅ 计算结果: {}", result_str);

                let data = serde_json::json!({
                    "expression": expression,
                    "result": result,
                    "result_str": result_str,
                    "result_type": "float", // Rust 计算结果为 f64
                });

                Ok(ToolResponse::success(
                    format!("计算结果: {}", result_str),
                    Some(data),
                    None,
                    None,
                ))
            }
            Err(e) => {
                let error_msg = format!("计算失败: {}", e);
                println!("❌ {}", error_msg);

                // 区分语法错误和一般执行错误
                let code = if format!("{}", e).contains("invalid") {
                    ToolErrorCode::InvalidFormat.as_str()
                } else {
                    ToolErrorCode::ExecutionError.as_str()
                };

                Ok(ToolResponse::error(
                    code,
                    &error_msg,
                    None,
                    Some(serde_json::json!({ "expression": expression })),
                ))
            }
        }
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new(
            "input",
            "string",
            "要计算的数学表达式，支持基本运算和数学函数",
            true,
            None,
        )]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
        })
    }
}

// =============================================================================
// 测试用例
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_arithmetic() {
        let tool = CalculatorTool::new();
        let resp = tool.run(json!({"input": "2 + 3 * 4"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        assert_eq!(resp.text, "计算结果: 14");
    }

    #[test]
    fn test_power_operator() {
        let tool = CalculatorTool::new();
        let resp = tool.run(json!({"input": "2 ** 10"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        assert_eq!(resp.text, "计算结果: 1024");
    }

    #[test]
    fn test_sqrt_function() {
        let tool = CalculatorTool::new();
        let resp = tool.run(json!({"input": "sqrt(16)"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        assert_eq!(resp.text, "计算结果: 4");
    }

    #[test]
    fn test_sin_with_pi() {
        let tool = CalculatorTool::new();
        let resp = tool.run(json!({"input": "sin(pi / 2)"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        // 由于浮点精度，结果可能非常接近 1
        let result: f64 = resp
            .data
            .get("result")
            .and_then(|v| v.as_f64())
            .unwrap();
        assert!((result - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_empty_expression_returns_error() {
        let tool = CalculatorTool::new();
        let resp = tool.run(json!({"input": ""})).unwrap();
        assert_eq!(resp.status, ToolStatus::Error);
        assert_eq!(
            resp.error_info.as_ref().unwrap().code,
            ToolErrorCode::InvalidParam.as_str()
        );
    }

    #[test]
    fn test_invalid_expression_returns_error() {
        let tool = CalculatorTool::new();
        let resp = tool.run(json!({"input": "2 +* 3"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Error);
        // 期望错误码为 InvalidFormat 或 ExecutionError
        let code = &resp.error_info.as_ref().unwrap().code;
        assert!(code == ToolErrorCode::InvalidFormat.as_str() || code == ToolErrorCode::ExecutionError.as_str());
    }

    #[test]
    fn test_expression_key_also_works() {
        let tool = CalculatorTool::new();
        let resp = tool.run(json!({"expression": "1 + 1"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        assert_eq!(resp.text, "计算结果: 2");
    }

    #[test]
    fn test_expression_eval() {
        let tool = CalculatorTool::new();
        let resp = tool.run(json!({"expression": "2+(2 ** 10)/4*(55-11^(4-2))"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        println!("计算结果: {}", resp.text);
    }
}