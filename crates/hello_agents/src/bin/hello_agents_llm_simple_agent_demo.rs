use std::collections::HashMap;
use anyhow::{Context, Result};
use hello_agents::agents::simple_agent::SimpleAgent;
use hello_agents::core::agent_trait::Agent;
use hello_agents::core::hello_agents_llm::HelloAgentsLLM;
use hello_agents::tools::builtin::calculator_tool::CalculatorTool;

// 示例：完整链路请求处理 Demo
//
// 运行前请设置以下环境变量：
//   LLM_MODEL_ID="deepseek-v4-flash"   # 或其它模型
//   LLM_API_KEY="your-api-key"
//   LLM_BASE_URL="https://api.deepseek.com/v1"   # 或 OpenAI 地址
//
// 运行: cargo run --example hello_agents_demo
fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    // 1. 创建 LLM 客户端（从环境变量读取配置，也可显式传入参数）
    //    参数签名：new(model, api_key, base_url, temperature, max_tokens, timeout, kwargs)
    //    这里全部使用 None，表示从环境变量自动加载。
    let llm = HelloAgentsLLM::new(
        Some("deepseek-v4-flash"),           // model: 从 LLM_MODEL_ID 环境变量读取
        None,           // api_key: 从 LLM_API_KEY 环境变量读取
        None,           // base_url: 从 LLM_BASE_URL 环境变量读取
        None,           // temperature: 使用默认值 0.7
        None,           // max_tokens: 不限制
        None,           // timeout: 默认 60s
        None,           // kwargs: 无额外参数
    )?;

    // 2. 创建 SimpleAgent（纯对话模式，无工具）
    let mut agent = SimpleAgent::new(
        "AI助手",
        llm,
        Some("你是一个有用的AI助手".to_string()),
        None,           // config: 使用默认配置
        None,           // tool_registry: 暂无工具
        false,          // enable_tool_calling: 关闭工具调用
        3,              // max_tool_iterations: 3（此时未启用，无影响）
    );

    // 3. 第一次对话（无工具）
    println!("🤖 正在与 LLM 对话...");
    let response = agent.run("你好!请介绍一下自己", HashMap::new())?;
    println!("✅ LLM 响应: {}", response);

    // 4. 添加计算器工具，并启用工具调用
    agent.add_tool(Box::new(CalculatorTool::new()), false);  // 注册计算器
    // 注意：add_tool 方法内部会自动启用 tool_registry 并设置 enable_tool_calling = true
    println!("\n📊 添加计算器工具列表: {:?}", agent.list_tools());
    // 5. 第二次对话（包含工具调用请求）
    println!("\n🔧 测试工具调用...");
    let response = agent.run("请帮我计算 2+3*4", HashMap::new())?;
    println!("✅ LLM 响应: {}", response);

    // 6. 查看工具调用统计（如果有）
    println!("\n📊 工具列表: {:?}", agent.list_tools());

    Ok(())
}
