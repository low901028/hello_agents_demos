use std::collections::HashMap;
use std::sync::Arc;

use hello_agents::agents::simple::SimpleAgent;
use hello_agents::core::traits::agent::Agent;
use hello_agents::core::types::exceptions::HelloAgentException;
use hello_agents::infra::llm::hello_agents_llm::HelloAgentsLLM;
use hello_agents::tools::builtin::calculator::CalculatorTool;

fn main() -> Result<(), HelloAgentException> {
    // 加载 .env 文件（可选）
    dotenvy::dotenv().ok();

    // 1. 创建 LLM 客户端（完全从环境变量读取配置）
    //    推荐：全部使用 None，让 HelloAgentsLLM 从 LLM_MODEL_ID / LLM_API_KEY / LLM_BASE_URL 自动加载
    let llm = HelloAgentsLLM::new(
        Some("deepseek-v4-flash"),   // model: 从环境变量读取
        None,   // api_key: 从环境变量读取
        None,   // base_url: 从环境变量读取
        None,   // temperature: 默认 0.7
        None,   // max_tokens: 不限制
        None,   // timeout: 默认 60s
        None,   // kwargs
    )?;

    // 2. 创建 SimpleAgent
    //    注意：SimpleAgent 需要 Arc<dyn LLMAdapter>，通过 llm.adapter() 获取共享引用
    let mut agent = SimpleAgent::new(
        "AI助手",
        llm.adapter(),                              // 返回 Arc<dyn LLMAdapter>
        Some("你是一个有用的AI助手".to_string()),
        Default::default(),                         // Config
        None,                                       // tool_registry 初始为空
        false,                                      // 暂不启用工具调用
        3,                                          // max_tool_iterations
    );

    // 3. 普通对话
    println!("🤖 正在与 LLM 对话...");
    let response = agent.run("你好!请介绍一下自己", HashMap::new())?;
    println!("✅ LLM 响应: {}", response);

    // 4. 添加计算器工具并启用工具调用
    agent.add_tool(Box::new(CalculatorTool::new()), false);
    println!("\n📊 当前工具列表: {:?}", agent.list_tools());

    // 5. 工具调用测试
    println!("\n🔧 测试工具调用...");
    let response = agent.run("请帮我计算 2+3*4", HashMap::new())?;
    println!("✅ LLM 响应: {}", response);

    Ok(())
}