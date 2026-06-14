// examples/custom_tools_demo.rs

use serde_json::{json, Value};

use hello_agents::core::traits::tool::{Tool, ToolParameter};
use hello_agents::core::types::exceptions::HelloAgentException;
use hello_agents::tools::error::ToolErrorCode;
use hello_agents::tools::registry::ToolRegistry;
use hello_agents::tools::response::{ToolResponse, ToolStatus};
use hello_agents::tools::tool_base::ToolBase;

// ============================================
// 示例 1：最简单的自定义工具 - GreetingTool
// ============================================

struct GreetingTool {
    base: ToolBase,
}

impl GreetingTool {
    fn new() -> Self {
        Self {
            base: ToolBase::new("greeting", "生成个性化的问候语", false),
        }
    }
}

impl Tool for GreetingTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let name = parameters.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "参数 'name' 不能为空",
                None,
                None,
            ));
        }
        let greeting = format!("你好，{}！欢迎使用 HelloAgents 框架！", name);
        Ok(ToolResponse::success(
            greeting.clone(),
            Some(json!({"name": name, "greeting": greeting.clone()})),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("name", "string", "要问候的人的名字", true, None)]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone() })
    }
}

// ============================================
// 示例 2：函数式工具（直接注册函数）
// ============================================

fn word_counter(text: &str) -> String {
    let count = text.split_whitespace().count();
    format!("文本包含 {} 个单词", count)
}

// ============================================
// 示例 3：可展开的多功能工具 - TextProcessorTool
// ============================================

/// 文本处理工具集（父工具，实际不执行，提供子工具）
struct TextProcessorTool {
    base: ToolBase,
}

impl TextProcessorTool {
    fn new() -> Self {
        Self {
            base: ToolBase::new("text_processor", "文本处理工具集，包含多种文本处理功能", true),
        }
    }
}

impl Tool for TextProcessorTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }
    fn run(&self, _parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        Ok(ToolResponse::error(
            ToolErrorCode::NotFound.as_str(),
            "请使用展开后的子工具: text_uppercase, text_lowercase, text_reverse",
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> { vec![] }

    fn is_expandable(&self) -> bool { true }

    fn get_tool_actions(&self) -> Vec<Box<dyn Tool>> {
        vec![
            Box::new(UppercaseTool::new()),
            Box::new(LowercaseTool::new()),
            Box::new(ReverseTool::new()),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone() })
    }
}

// --- 子工具：转大写 ---
struct UppercaseTool {
    base: ToolBase,
}

impl UppercaseTool {
    fn new() -> Self {
        Self { base: ToolBase::new("text_uppercase", "转换为大写", false) }
    }
}

impl Tool for UppercaseTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let text = parameters.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let result = text.to_uppercase();
        Ok(ToolResponse::success(
            format!("转换结果: {}", result),
            Some(json!({"original": text, "result": result})),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("text", "string", "要转换的文本", true, None)]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone() })
    }
}

// --- 子工具：转小写 ---
struct LowercaseTool {
    base: ToolBase,
}

impl LowercaseTool {
    fn new() -> Self {
        Self { base: ToolBase::new("text_lowercase", "转换为小写", false) }
    }
}

impl Tool for LowercaseTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let text = parameters.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let result = text.to_lowercase();
        Ok(ToolResponse::success(
            format!("转换结果: {}", result),
            Some(json!({"original": text, "result": result})),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("text", "string", "要转换的文本", true, None)]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone() })
    }
}

// --- 子工具：反转文本 ---
struct ReverseTool {
    base: ToolBase,
}

impl ReverseTool {
    fn new() -> Self {
        Self { base: ToolBase::new("text_reverse", "反转文本", false) }
    }
}

impl Tool for ReverseTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let text = parameters.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let result: String = text.chars().rev().collect();
        Ok(ToolResponse::success(
            format!("反转结果: {}", result),
            Some(json!({"original": text, "result": result})),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new("text", "string", "要反转的文本", true, None)]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self { base: self.base.clone() })
    }
}

// ============================================
// 主程序：演示所有工具的使用
// ============================================

fn main() -> Result<(), HelloAgentException> {
    println!("{}", "=".repeat(60));
    println!("HelloAgents 自定义工具完整示例");
    println!("{}", "=".repeat(60));
    println!();

    // 1. 创建工具注册表
    println!("📦 步骤 1: 创建工具注册表");
    let mut registry = ToolRegistry::new(None);
    println!("✅ 工具注册表创建成功");
    println!();

    // 2. 注册简单工具
    println!("📦 步骤 2: 注册简单工具");
    registry.register_tool(Box::new(GreetingTool::new()), false);
    println!();

    // 3. 注册函数式工具
    println!("📦 步骤 3: 注册函数式工具");
    registry.register_function(word_counter, Some("word_counter"), Some("统计文本中的单词数量"));
    println!();

    // 4. 注册可展开工具
    println!("📦 步骤 4: 注册可展开工具");
    registry.register_tool(Box::new(TextProcessorTool::new()), true);
    println!();

    // 5. 查看所有已注册的工具
    println!("📋 步骤 5: 查看所有已注册的工具");
    let tools = registry.list_tools();
    println!("已注册 {} 个工具:", tools.len());
    for tool_name in &tools {
        println!("  - {}", tool_name);
    }
    println!();

    // 6. 直接测试工具
    println!("{}", "=".repeat(60));
    println!("🧪 直接测试工具");
    println!("{}", "=".repeat(60));
    println!();

    // 测试问候工具
    println!("测试 1: 问候工具");
    let resp = registry.execute_tool("greeting", r#"{"name":"张三"}"#);
    println!("  状态: {:?}", resp.status);
    println!("  结果: {}", resp.text);
    println!();

    // 测试函数工具
    println!("测试 2: 单词计数工具");
    let resp = registry.execute_tool("word_counter", "Hello World from HelloAgents");
    println!("  状态: {:?}", resp.status);
    println!("  结果: {}", resp.text);
    println!();

    // 测试可展开工具的子工具
    println!("测试 3: 文本处理工具（大写）");
    let resp = registry.execute_tool("text_uppercase", r#"{"text":"hello world"}"#);
    println!("  状态: {:?}", resp.status);
    println!("  结果: {}", resp.text);
    println!();

    println!("测试 4: 文本处理工具（反转）");
    let resp = registry.execute_tool("text_reverse", r#"{"text":"HelloAgents"}"#);
    println!("  状态: {:?}", resp.status);
    println!("  结果: {}", resp.text);
    println!();

    // 7. 在 Agent 中使用（提示信息）
    println!("{}", "=".repeat(60));
    println!("🤖 在 Agent 中使用工具");
    println!("{}", "=".repeat(60));
    println!();
    println!("提示: 要在 Agent 中使用工具，需要配置 LLM。");
    println!("示例代码:");
    println!("  let llm = HelloAgentsLLM::new(None, None, None, None, None, None, None).unwrap();");
    println!("  let adapter = llm.adapter();");
    println!("  let agent = ReActAgent::new(\"assistant\", adapter, Some(registry), None, Config::default(), 5);");
    println!("  let result = agent.run(\"请用 greeting 工具问候李四\", HashMap::new());");
    println!();

    println!("{}", "=".repeat(60));
    println!("✅ 示例完成！");
    println!("{}", "=".repeat(60));

    Ok(())
}