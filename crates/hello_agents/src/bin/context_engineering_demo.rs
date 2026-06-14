// examples/context_engineering_demo.rs
// 上下文工程使用示例 (修复 UTF-8 边界错误)

use std::collections::HashMap;
use std::path::PathBuf;

use hello_agents::context::history::HistoryManager;
use hello_agents::context::token_counter::TokenCounter;
use hello_agents::context::truncator::{ObservationTruncator, TruncateDirection};
use hello_agents::core::types::message::{Message, MessageContent, MessageRole};

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

    let mut counter = TokenCounter::new("gpt-4");

    let msg1 = Message::new_text("Hello, world!", MessageRole::User);
    let tokens1 = counter.count_message(&msg1);
    println!("\n消息 1 Token 数: {}", tokens1);

    let tokens1_cached = counter.count_message(&msg1);
    println!("消息 1 Token 数（缓存）: {}", tokens1_cached);

    let messages = vec![
        Message::new_text("First message", MessageRole::User),
        Message::new_text("Second message", MessageRole::Assistant),
        Message::new_text("Third message", MessageRole::User),
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

    let mut manager = HistoryManager::new(3, 0.8);

    println!("\n添加对话历史...");
    for i in 0..5 {
        manager.append(Message::new_text(
            &format!("用户问题 {}", i + 1),
            MessageRole::User,
        ));
        manager.append(Message::new_text(
            &format!("助手回答 {}", i + 1),
            MessageRole::Assistant,
        ));
    }

    let history = manager.get_history();
    println!("总消息数: {}", history.len());

    let rounds = manager.estimate_rounds();
    let user_msgs = history
        .iter()
        .filter(|m| m.role == MessageRole::User)
        .count();
    let assistant_msgs = history
        .iter()
        .filter(|m| m.role == MessageRole::Assistant)
        .count();
    let summary = format!(
        "此会话包含 {} 轮对话：\n- 用户消息：{} 条\n- 助手消息：{} 条\n- 总消息数：{} 条\n\n（历史已压缩，保留最近 {} 轮完整对话）",
        rounds, user_msgs, assistant_msgs, history.len(), manager.min_retain_rounds
    );
    println!("\n简单摘要:\n{}", summary);

    println!("\n✅ 简单摘要测试完成");
}

fn demo_smart_summary() {
    println!("\n{}", "=".repeat(60));
    println!("示例 3: 智能摘要（可选，需额外 API）");
    println!("{}", "=".repeat(60));

    println!("\n智能摘要需要轻量 LLM 实例，本示例略过实际调用，仅展示流程。");
    println!("在 Python 示例中，启用了 enable_smart_compression，并通过 Agent 生成。");
    println!("Rust 版本可在 Agent 中配置并调用 _generate_smart_summary（需实现）。");

    println!("\n✅ 智能摘要测试完成（框架展示）");
}

fn demo_history_management() {
    println!("\n{}", "=".repeat(60));
    println!("示例 4: 历史消息管理");
    println!("{}", "=".repeat(60));

    let mut manager = HistoryManager::new(3, 0.8);

    println!("\n添加对话历史...");
    for i in 0..5 {
        manager.append(Message::new_text(
            &format!("用户问题 {}", i + 1),
            MessageRole::User,
        ));
        manager.append(Message::new_text(
            &format!("助手回答 {}", i + 1),
            MessageRole::Assistant,
        ));
    }

    println!("总消息数: {}", manager.get_history().len());
    println!("完整轮次数: {}", manager.estimate_rounds());

    println!("\n执行历史压缩...");
    manager.compress("前面讨论了一些基础问题");

    let compressed = manager.get_history();
    println!("压缩后消息数: {}", compressed.len());
    println!("第一条消息角色: {:?}", compressed[0].role);
    let content = match &compressed[0].content {
        Some(MessageContent::Text(t)) => t.clone(),
        _ => String::new(),
    };
    // 安全截断到 50 个字符
    let preview = safe_truncate(&content, 52);
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

    // 安全截断预览
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

    let mut manager = HistoryManager::new(5, 0.8);

    manager.append(Message::new_text("你好", MessageRole::User));
    manager.append(Message::new_text(
        "你好！有什么可以帮助你的？",
        MessageRole::Assistant,
    ));
    manager.append(Message::new_text("介绍一下你自己", MessageRole::User));
    manager.append(Message::new_text("我是 AI 助手", MessageRole::Assistant));

    println!("\n原始历史: {} 条消息", manager.get_history().len());

    let serialized = manager.to_dict();
    println!(
        "序列化数据: 历史数组长度 = {}",
        serialized
            .get("history")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    );

    let mut new_manager = HistoryManager::new(5, 0.8);
    new_manager.load_from_dict(&serialized);

    let restored_count = new_manager.get_history().len();
    println!("恢复后历史: {} 条消息", restored_count);

    if restored_count > 0 {
        let original = manager.get_history();
        let restored = new_manager.get_history();
        assert_eq!(original.len(), restored.len());
        assert_eq!(
            match &original[0].content {
                Some(MessageContent::Text(t)) => t.as_str(),
                _ => "",
            },
            match &restored[0].content {
                Some(MessageContent::Text(t)) => t.as_str(),
                _ => "",
            }
        );
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

    let mut manager = HistoryManager::new(5, 0.8);

    manager.append(Message::new_text("计算 2+3", MessageRole::User));
    manager.append(Message::new_text("我需要使用计算器", MessageRole::Assistant));
    manager.append(Message::new_text("5", MessageRole::Tool));
    manager.append(Message::new_text("结果是 5", MessageRole::Assistant));

    manager.append(Message::new_text("再算 10*2", MessageRole::User));
    manager.append(Message::new_text("使用计算器", MessageRole::Assistant));
    manager.append(Message::new_text("20", MessageRole::Tool));
    manager.append(Message::new_text("结果是 20", MessageRole::Assistant));

    println!("\n总消息数: {}", manager.get_history().len());
    println!("轮次边界: {:?}", manager.find_round_boundaries());
    println!("完整轮次数: {}", manager.estimate_rounds());

    println!("\n✅ 轮次边界检测完成");
}

fn main() {
    println!("\n🚀 上下文工程示例演示\n");

    demo_token_counter();
    demo_simple_summary();
    demo_smart_summary();
    demo_history_management();
    demo_observation_truncator();
    demo_session_serialization();
    demo_round_boundaries();

    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成！");
    println!("{}", "=".repeat(60));
}