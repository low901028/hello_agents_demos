// examples/demo_tool_response.rs
// 运行: cargo run --example demo_tool_response
/// """工具响应协议使用示例
///
/// 演示如何使用标准化的 ToolResponse 协议，包括：
/// - 成功响应 (SUCCESS)
/// - 部分成功响应 (PARTIAL)
/// - 错误响应 (ERROR)
/// - 标准错误码的使用
/// """
use std::collections::HashMap;

use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::{ToolResponse, ToolStatus};

/// 演示计算器工具 - 展示三种响应状态
struct DemoCalculatorTool;

impl DemoCalculatorTool {
    fn new() -> Self {
        Self
    }

    /// 验证表达式只包含合法字符
    fn is_valid_expression(expr: &str) -> bool {
        expr.chars()
            .all(|c| matches!(c, '0'..='9' | '+' | '-' | '*' | '/' | '(' | ')' | ' ' | '.'))
    }

    /// 简单的表达式求值（仅支持基本四则运算）
    fn evaluate(expr: &str) -> Result<f64, String> {
        // 清理表达式
        let expr = expr.replace(' ', "");

        // 处理括号和基本运算的简易求值器
        // 实际生产中使用 meval 或 evalexpr crate
        Self::simple_eval(&expr)
    }

    fn simple_eval(expr: &str) -> Result<f64, String> {
        // 使用递归下降解析基本表达式
        // 支持: +, -, *, /, 括号, 数字
        let tokens = Self::tokenize(expr)?;
        let (result, _) = Self::parse_expression(&tokens, 0)?;
        Ok(result)
    }

    fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        let mut chars = expr.chars().peekable();

        while let Some(&ch) = chars.peek() {
            match ch {
                '0'..='9' | '.' => {
                    let mut num = String::new();
                    while let Some(&c) = chars.peek() {
                        if c.is_ascii_digit() || c == '.' {
                            num.push(c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    tokens.push(Token::Number(
                        num.parse::<f64>().map_err(|e| format!("无效数字: {}", e))?
                    ));
                }
                '+' => { tokens.push(Token::Plus); chars.next(); }
                '-' => { tokens.push(Token::Minus); chars.next(); }
                '*' => { tokens.push(Token::Multiply); chars.next(); }
                '/' => { tokens.push(Token::Divide); chars.next(); }
                '(' => { tokens.push(Token::LParen); chars.next(); }
                ')' => { tokens.push(Token::RParen); chars.next(); }
                _ => return Err(format!("非法字符: {}", ch)),
            }
        }

        Ok(tokens)
    }

    fn parse_expression(tokens: &[Token], pos: usize) -> Result<(f64, usize), String> {
        let (mut left, mut pos) = Self::parse_term(tokens, pos)?;

        while pos < tokens.len() {
            match tokens[pos] {
                Token::Plus => {
                    let (right, new_pos) = Self::parse_term(tokens, pos + 1)?;
                    left += right;
                    pos = new_pos;
                }
                Token::Minus => {
                    let (right, new_pos) = Self::parse_term(tokens, pos + 1)?;
                    left -= right;
                    pos = new_pos;
                }
                _ => break,
            }
        }

        Ok((left, pos))
    }

    fn parse_term(tokens: &[Token], pos: usize) -> Result<(f64, usize), String> {
        let (mut left, mut pos) = Self::parse_factor(tokens, pos)?;

        while pos < tokens.len() {
            match tokens[pos] {
                Token::Multiply => {
                    let (right, new_pos) = Self::parse_factor(tokens, pos + 1)?;
                    left *= right;
                    pos = new_pos;
                }
                Token::Divide => {
                    let (right, new_pos) = Self::parse_factor(tokens, pos + 1)?;
                    if right == 0.0 {
                        return Err("除数不能为零".to_string());
                    }
                    left /= right;
                    pos = new_pos;
                }
                _ => break,
            }
        }

        Ok((left, pos))
    }

    fn parse_factor(tokens: &[Token], pos: usize) -> Result<(f64, usize), String> {
        if pos >= tokens.len() {
            return Err("表达式不完整".to_string());
        }

        match &tokens[pos] {
            Token::Number(n) => Ok((*n, pos + 1)),
            Token::Minus => {
                let (value, new_pos) = Self::parse_factor(tokens, pos + 1)?;
                Ok((-value, new_pos))
            }
            Token::LParen => {
                let (value, new_pos) = Self::parse_expression(tokens, pos + 1)?;
                if new_pos >= tokens.len() || !matches!(tokens[new_pos], Token::RParen) {
                    return Err("缺少右括号".to_string());
                }
                Ok((value, new_pos + 1))
            }
            _ => Err(format!("意外的 token: {:?}", tokens[pos])),
        }
    }
}

#[derive(Debug)]
enum Token {
    Number(f64),
    Plus,
    Minus,
    Multiply,
    Divide,
    LParen,
    RParen,
}

impl Tool for DemoCalculatorTool {
    fn name(&self) -> &str {
        "DemoCalculator"
    }

    fn description(&self) -> &str {
        "演示工具响应协议的计算器"
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("expression", "string", "数学表达式", true),
        ]
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let expression = parameters
            .get("expression")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        // 错误响应：参数无效
        if expression.is_empty() {
            return ToolResponse::error(
                ToolErrorCode::INVALID_PARAM,
                "表达式不能为空",
            );
        }

        // 错误响应：格式错误
        if !Self::is_valid_expression(&expression) {
            return ToolResponse::error(
                ToolErrorCode::INVALID_FORMAT,
                format!("表达式包含非法字符: {}", expression),
            );
        }

        // 尝试计算
        match Self::evaluate(&expression) {
            Ok(result) => {
                // 部分成功：结果过大（演示 PARTIAL 状态）
                if result.abs() > 1e10 {
                    return ToolResponse::partial(
                        format!("计算结果: {:.2e} (结果过大，已使用科学计数法)", result),
                    )
                        .with_data("result", serde_json::json!(result))
                        .with_data("expression", &*expression)
                        .with_data("truncated", true)
                        .with_data("reason", "结果超过显示范围");
                }

                // 成功响应
                ToolResponse::success(format!("计算结果: {}", result))
                    .with_data("result", serde_json::json!(result))
                    .with_data("expression", &*expression)
            }
            Err(e) => {
                // 错误响应：执行错误
                ToolResponse::error(
                    ToolErrorCode::EXECUTION_ERROR,
                    format!("计算失败: {}", e),
                )
            }
        }
    }
}

// ==================== 演示函数 ====================
fn demo_success_response() {
    println!("{}", "=".repeat(60));
    println!("示例 1: 成功响应 (SUCCESS)");
    println!("{}", "=".repeat(60));

    let tool = DemoCalculatorTool;
    let mut params = HashMap::new();
    params.insert("expression".into(), serde_json::json!("2 + 3 * 4"));

    let response = tool.run(&params);

    println!("\n状态: {:?}", response.status);
    println!("文本: {}", response.text);
    println!("数据: {:?}", response.data);

    assert_eq!(response.status, ToolStatus::Success);
    assert_eq!(response.data.get("result").and_then(|v| v.as_f64()), Some(14.0));
    println!("\n✅ 成功响应测试通过");
}

fn demo_partial_response() {
    println!();
    println!("{}", "=".repeat(60));
    println!("示例 2: 部分成功响应 (PARTIAL)");
    println!("{}", "=".repeat(60));

    let tool = DemoCalculatorTool;
    let mut params = HashMap::new();
    params.insert("expression".into(), serde_json::json!("10 ^ 15"));  // 10^15 超过 1e10 阈值

    let response = tool.run(&params);

    println!("\n状态: {:?}", response.status);
    println!("文本: {}", response.text);
    println!("数据: {:?}", response.data);
    println!(
        "原因: {}",
        response
            .data
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("N/A")
    );

    assert_eq!(response.status, ToolStatus::Partial);
    assert_eq!(
        response.data.get("truncated").and_then(|v| v.as_bool()),
        Some(true)
    );
    println!("\n✅ 部分成功响应测试通过");
}

fn demo_error_responses() {
    println!();
    println!("{}", "=".repeat(60));
    println!("示例 3: 错误响应 (ERROR)");
    println!("{}", "=".repeat(60));

    let tool = DemoCalculatorTool;

    // 错误 1: 参数无效
    println!("\n3.1 参数无效 (INVALID_PARAM)");
    let mut params = HashMap::new();
    params.insert("expression".into(), serde_json::json!(""));
    let response = tool.run(&params);
    println!("   状态: {:?}", response.status);
    if let Some(ref err) = response.error_info {
        println!("   错误码: {}", err.get("code").map(|s| s.as_str()).unwrap_or(""));
        println!("   错误消息: {}", err.get("message").map(|s| s.as_str()).unwrap_or(""));
        assert_eq!(err.get("code").map(|s| s.as_str()), Some(ToolErrorCode::INVALID_PARAM));
    }

    // 错误 2: 格式错误
    println!("\n3.2 格式错误 (INVALID_FORMAT)");
    let mut params = HashMap::new();
    params.insert("expression".into(), serde_json::json!("2 + abc"));
    let response = tool.run(&params);
    println!("   状态: {:?}", response.status);
    if let Some(ref err) = response.error_info {
        println!("   错误码: {}", err.get("code").map(|s| s.as_str()).unwrap_or(""));
        println!("   错误消息: {}", err.get("message").map(|s| s.as_str()).unwrap_or(""));
        assert_eq!(err.get("code").map(|s| s.as_str()), Some(ToolErrorCode::INVALID_FORMAT));
    }

    // 错误 3: 执行错误
    println!("\n3.3 执行错误 (EXECUTION_ERROR)");
    let mut params = HashMap::new();
    params.insert("expression".into(), serde_json::json!("1 / 0"));
    let response = tool.run(&params);
    println!("   状态: {:?}", response.status);
    if let Some(ref err) = response.error_info {
        println!("   错误码: {}", err.get("code").map(|s| s.as_str()).unwrap_or(""));
        println!("   错误消息: {}", err.get("message").map(|s| s.as_str()).unwrap_or(""));
        assert_eq!(err.get("code").map(|s| s.as_str()), Some(ToolErrorCode::EXECUTION_ERROR));
    }

    println!("\n✅ 所有错误响应测试通过");
}

// ==================== 主函数 ====================
#[tokio::main]
pub async fn main() -> anyhow::Result<()>{
    println!("🚀 工具响应协议示例");
    println!();

    demo_success_response();
    demo_partial_response();
    demo_error_responses();

    println!();
    println!("{}", "=".repeat(60));
    println!("✅ 所有示例运行完成！");
    println!("{}", "=".repeat(60));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_calculator_success() {
        let tool = DemoCalculatorTool;
        let mut params = HashMap::new();
        params.insert("expression".into(), serde_json::json!("2 + 3 * 4"));
        let response = tool.run(&params);
        assert_eq!(response.status, ToolStatus::Success);
        assert_eq!(response.data.get("result").and_then(|v| v.as_f64()), Some(14.0));
    }

    #[test]
    fn test_demo_calculator_empty_expression() {
        let tool = DemoCalculatorTool;
        let mut params = HashMap::new();
        params.insert("expression".into(), serde_json::json!(""));
        let response = tool.run(&params);
        assert_eq!(response.status, ToolStatus::Error);
        assert_eq!(
            response.error_info.as_ref().and_then(|e| e.get("code").cloned()),
            Some(ToolErrorCode::INVALID_PARAM.to_string())
        );
    }

    #[test]
    fn test_demo_calculator_invalid_format() {
        let tool = DemoCalculatorTool;
        let mut params = HashMap::new();
        params.insert("expression".into(), serde_json::json!("2 + abc"));
        let response = tool.run(&params);
        assert_eq!(response.status, ToolStatus::Error);
        assert_eq!(
            response.error_info.as_ref().and_then(|e| e.get("code").cloned()),
            Some(ToolErrorCode::INVALID_FORMAT.to_string())
        );
    }

    #[test]
    fn test_demo_calculator_division_by_zero() {
        let tool = DemoCalculatorTool;
        let mut params = HashMap::new();
        params.insert("expression".into(), serde_json::json!("1 / 0"));
        let response = tool.run(&params);
        assert_eq!(response.status, ToolStatus::Error);
        assert_eq!(
            response.error_info.as_ref().and_then(|e| e.get("code").cloned()),
            Some(ToolErrorCode::EXECUTION_ERROR.to_string())
        );
    }
}