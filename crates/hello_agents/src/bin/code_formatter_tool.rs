// examples/code_formatter_tool.rs
// 代码格式化工具 - 对应 Python CodeFormatterTool

use std::collections::HashMap;

use serde_json::Value;

use hello_agents::core::traits::tool::{Tool, ToolParameter};
use hello_agents::core::types::exceptions::HelloAgentException;
use hello_agents::tools::error::ToolErrorCode;
use hello_agents::tools::response::{ToolResponse, ToolStatus};
use hello_agents::tools::tool_base::ToolBase;

/// 代码格式化工具
pub struct CodeFormatterTool {
    base: ToolBase,
}

impl CodeFormatterTool {
    pub fn new() -> Self {
        Self {
            base: ToolBase::new(
                "code_formatter",
                "格式化 Python 代码，支持自定义缩进和行宽",
                false,
            ),
        }
    }

    /// 核心格式化逻辑
    fn format_code(
        &self,
        code: &str,
        indent: usize,
        max_line_length: usize,
        fix_imports: bool,
    ) -> Result<(String, Vec<String>), String> {
        let mut changes: Vec<String> = Vec::new();
        let lines: Vec<&str> = code.lines().collect();
        let mut formatted_lines: Vec<String> = Vec::new();

        // 1. 修复缩进
        for line in &lines {
            let stripped = line.trim_start();
            if stripped.is_empty() {
                formatted_lines.push(String::new());
                continue;
            }
            let current_indent_len = line.len() - stripped.len();
            let indent_level = current_indent_len / indent;
            let new_line = format!("{}{}", " ".repeat(indent_level * indent), stripped);
            formatted_lines.push(new_line);
        }

        if formatted_lines != lines.iter().map(|s| s.to_string()).collect::<Vec<_>>() {
            changes.push("修复缩进".to_string());
        }

        // 2. 修复 import 语句
        if fix_imports {
            let import_fixed = self.fix_imports(&formatted_lines);
            if import_fixed != formatted_lines {
                formatted_lines = import_fixed;
                changes.push("整理 import 语句".to_string());
            }
        }

        // 3. 移除多余空行
        let cleaned = self.remove_extra_blank_lines(&formatted_lines);
        if cleaned != formatted_lines {
            formatted_lines = cleaned;
            changes.push("移除多余空行".to_string());
        }

        // 4. 检查行宽（仅警告）
        let long_lines: Vec<usize> = formatted_lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.len() > max_line_length)
            .map(|(i, _)| i + 1)
            .collect();
        if !long_lines.is_empty() {
            changes.push(format!("检测到 {} 行超过最大行宽", long_lines.len()));
        }

        let formatted_code = formatted_lines.join("\n");
        Ok((formatted_code, changes))
    }

    /// 整理 import 语句
    fn fix_imports(&self, lines: &[String]) -> Vec<String> {
        let mut import_lines: Vec<String> = Vec::new();
        let mut from_import_lines: Vec<String> = Vec::new();
        let mut other_lines: Vec<String> = Vec::new();

        for line in lines {
            let trimmed = line.trim();
            if trimmed.starts_with("import ") {
                import_lines.push(line.clone());
            } else if trimmed.starts_with("from ") {
                from_import_lines.push(line.clone());
            } else {
                other_lines.push(line.clone());
            }
        }

        import_lines.sort();
        from_import_lines.sort();

        let mut result: Vec<String> = Vec::new();
        if !import_lines.is_empty() {
            result.extend(import_lines);
        }
        if !from_import_lines.is_empty() {
            if !result.is_empty() {
                result.push(String::new()); // 空行分隔
            }
            result.extend(from_import_lines);
        }
        if !other_lines.is_empty() {
            if !result.is_empty() {
                result.push(String::new());
                result.push(String::new()); // 两个空行分隔
            }
            result.extend(other_lines);
        }
        result
    }

    /// 移除多余空行（最多保留两个连续空行）
    fn remove_extra_blank_lines(&self, lines: &[String]) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        let mut blank_count = 0;

        for line in lines {
            if line.trim().is_empty() {
                blank_count += 1;
                if blank_count <= 2 {
                    result.push(line.clone());
                }
            } else {
                blank_count = 0;
                result.push(line.clone());
            }
        }
        result
    }
}

impl Tool for CodeFormatterTool {
    fn name(&self) -> &str {
        &self.base.name
    }

    fn description(&self) -> &str {
        &self.base.description
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        // 1. 参数验证
        let code = parameters
            .get("code")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if code.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "参数 'code' 不能为空",
                None,
                None,
            ));
        }

        let indent = parameters
            .get("indent")
            .and_then(|v| v.as_u64())
            .unwrap_or(4) as usize;
        if !(1..=8).contains(&indent) {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "参数 'indent' 必须是 1-8 之间的整数",
                None,
                None,
            ));
        }

        let max_line_length = parameters
            .get("max_line_length")
            .and_then(|v| v.as_u64())
            .unwrap_or(80) as usize;
        if max_line_length < 40 {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "参数 'max_line_length' 必须大于等于 40",
                None,
                None,
            ));
        }

        let fix_imports = parameters
            .get("fix_imports")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // 2. 执行格式化
        match self.format_code(&code, indent, max_line_length, fix_imports) {
            Ok((formatted_code, changes)) => {
                let text = if changes.is_empty() {
                    "代码已经符合格式规范，无需修改".to_string()
                } else {
                    format!("代码格式化完成，应用了以下修改: {}", changes.join(", "))
                };

                let original_lines = code.lines().count();
                let formatted_lines = formatted_code.lines().count();

                Ok(ToolResponse::success(
                    text,
                    Some(serde_json::json!({
                        "original_code": code,
                        "formatted_code": formatted_code,
                        "changes": changes,
                        "original_lines": original_lines,
                        "formatted_lines": formatted_lines,
                    })),
                    Some(serde_json::json!({
                        "changes_count": changes.len()
                    })),
                    None,
                ))
            }
            Err(e) => {
                // 区分语法错误和一般执行错误
                let code = if e.contains("语法错误") {
                    ToolErrorCode::InvalidFormat.as_str()
                } else {
                    ToolErrorCode::ExecutionError.as_str()
                };
                Ok(ToolResponse::error(code, &e, None, Some(serde_json::json!({"code": code}))))
            }
        }
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("code", "string", "要格式化的 Python 代码", true, None),
            ToolParameter::new(
                "indent",
                "integer",
                "缩进空格数（1-8）",
                false,
                Some(Value::Number(serde_json::Number::from(4))),
            ),
            ToolParameter::new(
                "max_line_length",
                "integer",
                "最大行宽（>=40）",
                false,
                Some(Value::Number(serde_json::Number::from(80))),
            ),
            ToolParameter::new(
                "fix_imports",
                "boolean",
                "是否自动修复 import 语句",
                false,
                Some(Value::Bool(true)),
            ),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
        })
    }
}

// ============================================
// 使用示例
// ============================================
fn main() -> Result<(), HelloAgentException> {
    // 1. 创建工具
    println!("=== 创建代码格式化工具 ===");
    let tool = CodeFormatterTool::new();

    // 2. 测试基本格式化
    println!("\n=== 测试基本格式化 ===");
    let messy_code = r#"
import os
from typing import Dict
import sys


def hello(  ):
        print(  'hello'  )


class MyClass:
  def __init__(self):
    self.value=42
"#;
    let resp = tool.run(serde_json::json!({
        "code": messy_code,
        "indent": 4,
        "max_line_length": 80,
    }))?;
    println!("状态: {:?}", resp.status);
    println!("变更: {:?}", resp.data);
    println!("格式化后的代码:\n{}", resp.data["formatted_code"].as_str().unwrap_or(""));

    // 3. 测试错误处理
    println!("\n=== 测试错误处理 ===");
    let resp2 = tool.run(serde_json::json!({"code": ""}))?;
    println!("状态: {:?}", resp2.status);
    if let Some(err) = &resp2.error_info {
        println!("错误: {} - {}", err.code, err.message);
    }

    Ok(())
}