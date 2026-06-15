// examples/context_engineering_demo.rs
// 上下文工程使用示例（适配最新架构，异步 main）

use std::path::PathBuf;
use std::sync::Arc;
use hello_agents::context::history_manager_impl::HistoryManagerImpl;
use hello_agents::context::token_counter::TokenCounter;
use hello_agents::context::truncator::{ObservationTruncator, TruncateDirection};
use hello_agents::core::traits::history::HistoryManager;
use hello_agents::core::traits::llm_provider;
use hello_agents::core::traits::llm_provider::LlmProvider;
use hello_agents::core::types::message::Message;
use hello_agents::infra::openai_adapter::OpenAIAdapter;

/// 安全的字符切片，确保不会截断多字节字符
fn safe_truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

fn demo_token_counter() {
    println!("{}", "=".repeat(60));
    println!("示例 1: Token 计数器（缓存 + 增量计算）");
    println!("{}", "=".repeat(60));

    let mut counter = TokenCounter::new("deepseek-v4-chat");

    let msg1 = Message::user("Hello, world!");
    let tokens1 = counter.count_message(&msg1);
    println!("\n消息 1 Token 数: {}", tokens1);

    let tokens1_cached = counter.count_message(&msg1);
    println!("消息 1 Token 数（缓存）: {}", tokens1_cached);

    let messages = vec![
        Message::user("First message"),
        Message::assistant("Second message"),
        Message::user("Third message"),
    ];
    let total_tokens = counter.count_messages(&messages);
    println!("\n消息列表总 Token 数: {}", total_tokens);

    let stats = counter.get_cache_stats();
    println!("\n缓存统计: {:?}", stats);

    println!("\n✅ Token 计数器测试完成");
}

fn demo_simple_summary() {
    println!("\n{}", "=".repeat(60));
    println!("示例 2: 简单摘要（默认，无需额外 API）");
    println!("{}", "=".repeat(60));

    let mut manager = HistoryManagerImpl::new(3, 0.8);

    println!("\n添加对话历史...");
    for i in 0..5 {
        manager.add_message(Message::user(&format!("用户问题 {}", i + 1)));
        manager.add_message(Message::assistant(&format!("助手回答 {}", i + 1)));
    }

    let history = manager.messages();
    println!("总消息数: {}", history.len());

    let rounds = manager.estimate_rounds();
    let user_msgs = history.iter().filter(|m| m.role == "user").count();
    let assistant_msgs = history.iter().filter(|m| m.role == "assistant").count();
    let summary = format!(
        "此会话包含 {} 轮对话：\n- 用户消息：{} 条\n- 助手消息：{} 条\n- 总消息数：{} 条\n\n（历史已压缩，保留最近 {} 轮完整对话）",
        rounds, user_msgs, assistant_msgs, history.len(), manager.min_retain_rounds
    );
    println!("\n简单摘要:\n{}", summary);

    println!("\n✅ 简单摘要测试完成");
}

async fn demo_smart_summary() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n{}", "=".repeat(60));
    println!("示例 3: 智能摘要（调用 LLM）");
    println!("{}", "=".repeat(60));

    // 从环境变量获取 LLM 配置（与主 Demo 一致）
    let model = std::env::var("LLM_MODEL_ID").unwrap_or_else(|_| "deepseek-chat".to_string());
    let api_key = std::env::var("LLM_API_KEY")
        .expect("请设置环境变量 LLM_API_KEY");
    let base_url = std::env::var("LLM_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com/v1".to_string());

    // 创建 LLM 适配器
    let llm = Arc::new(OpenAIAdapter::new(&api_key, &base_url, &model));

    // 创建简单的历史管理器（不需要完整的 AgentRuntime）
    let mut history = HistoryManagerImpl::new(3, 0.8);

    // 模拟多轮对话
    println!("\n添加对话历史...");
    let messages = vec![
        Message::user("帮我分析这个项目的架构"),
        Message::assistant("好的，我会分析项目架构"),
        Message::user("发现了什么问题？"),
        Message::assistant("发现了一些架构问题，需要重构"),
        Message::user("继续分析"),
        Message::assistant("正在深入分析中"),
    ];
    for msg in &messages {
        history.add_message(msg.clone());
    }

    println!("总消息数: {}", history.messages().len());
    println!("Token 计数: {}", history.estimate_tokens());

    // 构建智能摘要 Prompt（与 Python 一致）
    let history_text: String = messages
        .iter()
        .map(|msg| {
            let content = msg.content.clone().unwrap_or_default();
            format!("[{}]: {}", msg.role, content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let summary_prompt = format!("请将以下对话历史压缩为结构化摘要，保留关键信息：\n\n## 对话历史\n{history_text}\n\n## 摘要要求\n1. **任务目标**：用户想要完成什么？\n2. **关键决策**：做了哪些重要决定？\n3. **已完成工作**：完成了哪些任务？（列表形式）\n4. **待处理事项**：还有什么未完成？\n5. **重要发现**：有哪些关键信息或问题？\n\n请用简洁的中文输出，每部分不超过 3 行。");

    println!("\n生成智能摘要（调用 LLM）...");

    let chat_messages = vec![
        Message::system("你是一个专业的对话摘要助手，擅长提取关键信息。"),
        Message::user(&summary_prompt),
    ];

    // 调用 LLM
    let resp = llm.chat(&chat_messages, None, None).await?;
    let summary = resp.content.unwrap_or_default();

    println!("\n智能摘要:\n{}", summary);
    println!("\n✅ 智能摘要测试完成");

    Ok(())
}

fn demo_history_management() {
    println!("\n{}", "=".repeat(60));
    println!("示例 4: 历史消息管理");
    println!("{}", "=".repeat(60));

    let mut manager = HistoryManagerImpl::new(3, 0.8);

    println!("\n添加对话历史...");
    for i in 0..5 {
        manager.add_message(Message::user(&format!("用户问题 {}", i + 1)));
        manager.add_message(Message::assistant(&format!("助手回答 {}", i + 1)));
    }

    println!("总消息数: {}", manager.messages().len());
    println!("完整轮次数: {}", manager.estimate_rounds());

    println!("\n执行历史压缩...");
    manager.compress("前面讨论了一些基础问题");

    let compressed = manager.messages();
    println!("压缩后消息数: {}", compressed.len());
    println!("第一条消息角色: {:?}", compressed[0].role);
    let content = compressed[0].content.clone().unwrap_or_default();
    let preview = safe_truncate(&content, 50);
    println!("摘要内容: {}...", preview);

    println!("\n✅ 历史管理测试完成");
}

fn demo_observation_truncator() {
    println!("\n{}", "=".repeat(60));
    println!("示例 5: 工具输出截断");
    println!("{}", "=".repeat(60));

    let tmp_dir = tempfile::tempdir().expect("创建临时目录失败");
    let truncator = ObservationTruncator::new(10, 500, TruncateDirection::Head, tmp_dir.path());

    let long_output = (1..=100)
        .map(|i| format!("Line {}: Some content here", i))
        .collect::<Vec<_>>()
        .join("\n");

    println!(
        "\n原始输出: {} 字节, {} 行",
        long_output.len(),
        long_output.lines().count()
    );

    let result = truncator.truncate("search_tool", &long_output, None);

    println!("\n截断状态: {}", result.truncated);
    println!("预览长度: {} 字节", result.preview.len());
    println!("保存路径: {:?}", result.full_output_path);

    let preview = safe_truncate(&result.preview, 200);
    println!("\n预览内容:\n{}...", preview);

    if let Some(path) = result.full_output_path {
        assert!(PathBuf::from(&path).exists());
        println!("\n✅ 完整输出已保存到: {}", path);
    }
}

fn demo_session_serialization() {
    println!("\n{}", "=".repeat(60));
    println!("示例 6: 会话序列化/反序列化");
    println!("{}", "=".repeat(60));

    let mut manager = HistoryManagerImpl::new(5, 0.8);

    manager.add_message(Message::user("你好"));
    manager.add_message(Message::assistant("你好！有什么可以帮助你的？"));
    manager.add_message(Message::user("介绍一下你自己"));
    manager.add_message(Message::assistant("我是 AI 助手"));

    println!("\n原始历史: {} 条消息", manager.messages().len());

    let serialized = manager.to_dict();
    println!(
        "序列化数据: 历史数组长度 = {}",
        serialized
            .get("history")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    );

    let mut new_manager = HistoryManagerImpl::new(5, 0.8);
    new_manager.load_from_dict(&serialized);

    let restored_count = new_manager.messages().len();
    println!("恢复后历史: {} 条消息", restored_count);

    if restored_count > 0 {
        let original = manager.messages();
        let restored = new_manager.messages();
        assert_eq!(original.len(), restored.len());
        assert_eq!(original[0].content, restored[0].content);
        println!("\n✅ 会话序列化测试完成");
    } else {
        eprintln!("❌ 反序列化失败，请检查 Message 的 Serialize/Deserialize 实现");
        eprintln!(
            "序列化JSON: {}",
            serde_json::to_string_pretty(&serialized).unwrap()
        );
    }
}

fn demo_round_boundaries() {
    println!("\n{}", "=".repeat(60));
    println!("示例 7: 轮次边界检测");
    println!("{}", "=".repeat(60));

    let mut manager = HistoryManagerImpl::new(5, 0.8);

    manager.add_message(Message::user("计算 2+3"));
    manager.add_message(Message::assistant("我需要使用计算器"));
    manager.add_message(Message::tool("1".into(), "5"));
    manager.add_message(Message::assistant("结果是 5"));

    manager.add_message(Message::user("再算 10*2"));
    manager.add_message(Message::assistant("使用计算器"));
    manager.add_message(Message::tool("2".into(), "20"));
    manager.add_message(Message::assistant("结果是 20"));

    println!("\n总消息数: {}", manager.messages().len());
    println!("轮次边界: {:?}", manager.find_round_boundaries());
    println!("完整轮次数: {}", manager.estimate_rounds());

    println!("\n✅ 轮次边界检测完成");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    println!("\n🚀 上下文工程示例演示\n");

    demo_token_counter();
    demo_simple_summary();
    demo_smart_summary().await.unwrap();
    demo_history_management();
    demo_observation_truncator();
    demo_session_serialization();
    demo_round_boundaries();

    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成！");
    println!("{}", "=".repeat(60));

    Ok(())
}