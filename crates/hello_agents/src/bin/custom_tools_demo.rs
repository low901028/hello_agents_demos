// examples/custom_tools_demo.rs
// 基于最新 async 架构的自定义工具示例

use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};

use hello_agents::core::traits::tool::Tool;
use hello_agents::core::traits::expandable::ExpandableTool;
use hello_agents::core::traits::tool_registry::ToolRegistry;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::core::types::response::ToolResponse;
use hello_agents::infra::tool_registry_impl::ToolRegistryImpl;

// ============================================
// 示例 1：最简单的自定义工具 - GreetingTool
// ============================================

struct GreetingTool;

#[async_trait]
impl Tool for GreetingTool {
    fn name(&self) -> &str { "greeting" }
    fn description(&self) -> &str { "生成个性化的问候语" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "要问候的人的名字"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "参数 'name' 不能为空"));
        }
        let greeting = format!("你好，{}！欢迎使用 HelloAgents 框架！", name);
        Ok(ToolResponse::success(greeting))
    }
}

// ============================================
// 示例 2：函数式工具（包装为 Tool）
// ============================================

struct WordCounter;

#[async_trait]
impl Tool for WordCounter {
    fn name(&self) -> &str { "word_counter" }
    fn description(&self) -> &str { "统计文本中的单词数量" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "要统计的文本"
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let count = text.split_whitespace().count();
        let result = format!("文本包含 {} 个单词", count);
        Ok(ToolResponse::success(result))
    }
}

// ============================================
// 示例 3：可展开的多功能工具 - TextProcessorTool
// ============================================

struct TextProcessorTool;

#[async_trait]
impl Tool for TextProcessorTool {
    fn name(&self) -> &str { "text_processor" }
    fn description(&self) -> &str { "文本处理工具集，包含多种文本处理功能" }
    fn parameters(&self) -> Value { json!({}) }

    async fn execute(&self, _args: Value) -> Result<ToolResponse, HelloAgentError> {
        Ok(ToolResponse::error("NOT_IMPLEMENTED", "请使用展开后的子工具"))
    }
}

impl ExpandableTool for TextProcessorTool {
    fn expand(&self) -> Vec<Box<dyn Tool>> {
        vec![
            Box::new(UppercaseTool),
            Box::new(LowercaseTool),
            Box::new(ReverseTool),
        ]
    }
}

// --- 子工具：转大写 ---
struct UppercaseTool;

#[async_trait]
impl Tool for UppercaseTool {
    fn name(&self) -> &str { "text_uppercase" }
    fn description(&self) -> &str { "转换为大写" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "要转换的文本"
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let result = text.to_uppercase();
        Ok(ToolResponse::success(format!("转换结果: {}", result)))
    }
}

// --- 子工具：转小写 ---
struct LowercaseTool;

#[async_trait]
impl Tool for LowercaseTool {
    fn name(&self) -> &str { "text_lowercase" }
    fn description(&self) -> &str { "转换为小写" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "要转换的文本"
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let result = text.to_lowercase();
        Ok(ToolResponse::success(format!("转换结果: {}", result)))
    }
}

// --- 子工具：反转文本 ---
struct ReverseTool;

#[async_trait]
impl Tool for ReverseTool {
    fn name(&self) -> &str { "text_reverse" }
    fn description(&self) -> &str { "反转文本" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "要反转的文本"
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let result: String = text.chars().rev().collect();
        Ok(ToolResponse::success(format!("反转结果: {}", result)))
    }
}

// ============================================
// 主程序：演示所有工具的使用（异步）
// ============================================

#[tokio::main]
async fn main() -> Result<(), HelloAgentError> {
    println!("{}", "=".repeat(60));
    println!("HelloAgents 自定义工具完整示例 (Async)");
    println!("{}", "=".repeat(60));
    println!();

    // 1. 创建工具注册表
    println!("📦 步骤 1: 创建工具注册表");
    let mut registry = ToolRegistryImpl::new();
    println!("✅ 工具注册表创建成功");
    println!();

    // 2. 注册简单工具
    println!("📦 步骤 2: 注册简单工具");
    registry.register(Box::new(GreetingTool));
    println!();

    // 3. 注册函数式工具
    println!("📦 步骤 3: 注册函数式工具");
    registry.register(Box::new(WordCounter));
    println!();

    // 4. 注册可展开工具
    println!("📦 步骤 4: 注册可展开工具");
    registry.register_expandable(Box::new(TextProcessorTool));
    println!();

    // 5. 查看所有已注册的工具
    println!("📋 步骤 5: 查看所有已注册的工具");
    let tools = registry.list_tools();
    println!("已注册 {} 个工具:", tools.len());
    for tool_name in &tools {
        println!("  - {}", tool_name);
    }
    println!();

    // 6. 直接测试工具（异步执行）
    println!("{}", "=".repeat(60));
    println!("🧪 直接测试工具");
    println!("{}", "=".repeat(60));
    println!();

    // 测试问候工具
    println!("测试 1: 问候工具");
    let resp = registry.execute("greeting", json!({"name": "张三"})).await?;
    println!("  状态: {:?}", resp.status);
    println!("  结果: {}", resp.text);
    println!();

    // 测试单词计数工具
    println!("测试 2: 单词计数工具");
    let resp = registry.execute("word_counter", json!({"text": "Hello World from HelloAgents"})).await?;
    println!("  状态: {:?}", resp.status);
    println!("  结果: {}", resp.text);
    println!();

    // 测试可展开工具的子工具
    println!("测试 3: 文本处理工具（大写）");
    let resp = registry.execute("text_uppercase", json!({"text": "hello world"})).await?;
    println!("  状态: {:?}", resp.status);
    println!("  结果: {}", resp.text);
    println!();

    println!("测试 4: 文本处理工具（反转）");
    let resp = registry.execute("text_reverse", json!({"text": "HelloAgents"})).await?;
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
    println!("  let llm = Arc::new(OpenAIAdapter::new(...));");
    println!("  let tools = Arc::new(ToolRegistryImpl::new());");
    println!("  tools.register(Box::new(GreetingTool));");
    println!("  let runtime = AgentRuntime::new(llm, tools, ...);");
    println!("  let mut agent = SimpleAgent::new(...);");
    println!("  let result = agent.run(\"请用 greeting 工具问候李四\", &runtime).await?;");
    println!();

    println!("{}", "=".repeat(60));
    println!("✅ 示例完成！");
    println!("{}", "=".repeat(60));

    Ok(())
}