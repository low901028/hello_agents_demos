// examples/devlog_demo.rs
// DevLogTool 使用示例

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::json;

use hello_agents::agents::react::ReActAgent;
use hello_agents::core::traits::agent::Agent;
use hello_agents::core::traits::tool::Tool;
use hello_agents::core::types::config::Config;
use hello_agents::infra::llm::hello_agents_llm::HelloAgentsLLM;
use hello_agents::tools::builtin::devlog::DevLogTool;
use hello_agents::tools::registry::ToolRegistry;

fn demo_1_basic_operations() {
    println!("{}", "=".repeat(60));
    println!("示例 1：DevLogTool 基本操作");
    println!("{}", "=".repeat(60));

    // 创建 DevLogTool（使用相对路径）
    let tool = DevLogTool::new(
        "demo-session-001",
        "DemoAgent",
        PathBuf::from("."),
        PathBuf::from("memory/devlogs"),
    );

    println!("\n✅ DevLogTool 已创建");
    println!("   会话 ID: {}", tool.session_id);
    println!("   Agent: {}", tool.agent_name);

    // 追加决策日志
    println!("\n📝 追加决策日志...");
    let response = tool.run(json!({
        "action": "append",
        "category": "decision",
        "content": "选择使用 Redis 作为缓存层，因为需要支持分布式部署和高并发访问",
        "metadata": {
            "tags": ["architecture", "cache", "redis"],
            "step": 3,
            "related_tool": "WriteTool"
        }
    }));
    println!("   {}", response.unwrap().text);

    // 追加问题日志
    println!("\n📝 追加问题日志...");
    let response = tool.run(json!({
        "action": "append",
        "category": "issue",
        "content": "API 响应时间超过 2 秒，影响用户体验",
        "metadata": {
            "tags": ["performance", "api"],
            "severity": "high"
        }
    }));
    println!("   {}", response.unwrap().text);

    // 追加解决方案日志
    println!("\n📝 追加解决方案日志...");
    let response = tool.run(json!({
        "action": "append",
        "category": "solution",
        "content": "增加 Redis 缓存，缓存热点数据，减少数据库查询",
        "metadata": {
            "tags": ["performance", "cache"],
            "related_issue": "API 响应时间超过 2 秒"
        }
    }));
    println!("   {}", response.unwrap().text);

    // 生成摘要
    println!("\n📊 生成摘要...");
    let response = tool.run(json!({"action": "summary"}));
    println!("   {}", response.unwrap().text);

    // 读取所有日志
    println!("\n📖 读取所有日志...");
    let response = tool.run(json!({"action": "read"}));
    println!("{}", response.unwrap().text);
}

fn demo_2_filtering() {
    println!("\n{}", "=".repeat(60));
    println!("示例 2：过滤查询");
    println!("{}", "=".repeat(60));

    let tool = DevLogTool::new(
        "demo-session-002",
        "DemoAgent",
        PathBuf::from("."),
        PathBuf::from("memory/devlogs"),
    );

    // 添加多条日志（必须包含 action: "append"）
    let logs = vec![
        json!({"action": "append", "category": "decision", "content": "使用 PostgreSQL 作为主数据库", "metadata": {"tags": ["database"]}}),
        json!({"action": "append", "category": "decision", "content": "使用 Redis 作为缓存", "metadata": {"tags": ["cache"]}}),
        json!({"action": "append", "category": "issue", "content": "数据库连接池耗尽", "metadata": {"tags": ["database", "performance"]}}),
        json!({"action": "append", "category": "solution", "content": "增加连接池大小到 50", "metadata": {"tags": ["database"]}}),
        json!({"action": "append", "category": "refactor", "content": "重构用户认证模块", "metadata": {"tags": ["auth", "security"]}}),
    ];

    for log in logs {
        tool.run(log);
    }

    println!("\n✅ 已添加 {} 条日志", 5);

    // 按类别过滤
    println!("\n🔍 只查看决策类日志...");
    let response = tool.run(json!({
        "action": "read",
        "filter": {"category": "decision"}
    }));
    println!("{}", response.unwrap().text);

    // 按标签过滤
    println!("\n🔍 只查看数据库相关日志...");
    let response = tool.run(json!({
        "action": "read",
        "filter": {"tags": ["database"]}
    }));
    println!("{}", response.unwrap().text);

    // 限制数量
    println!("\n🔍 只查看最近 2 条日志...");
    let response = tool.run(json!({
        "action": "read",
        "filter": {"limit": 2}
    }));
    println!("{}", response.unwrap().text);
}

fn demo_3_agent_integration() {
    println!("\n{}", "=".repeat(60));
    println!("示例 3：Agent 集成 - 零配置使用");
    println!("{}", "=".repeat(60));

    // 配置启用 DevLog
    let config = Config {
        devlog_enabled: true,
        devlog_persistence_dir: "memory/devlogs".into(),
        trace_enabled: false,
        session_enabled: false,
        todowrite_enabled: false,
        subagent_enabled: false,
        skills_enabled: false,
        ..Default::default()
    };

    // 创建 LLM 和注册表
    let llm = HelloAgentsLLM::new(Some("deepseek-v4-flash"), None, None, None, None, None, None)
        .expect("创建 LLM 失败");
    let adapter = llm.adapter();
    let mut registry = ToolRegistry::new(None);

    // 创建 Agent（在 Rust 版本中，DevLogTool 不会自动注册，需手动添加）
    let devlog_tool = DevLogTool::new(
        "auto-session",
        "开发助手",
        PathBuf::from("."),
        PathBuf::from("memory/devlogs"),
    );
    registry.register_tool(Box::new(devlog_tool), false);

    let agent = ReActAgent::new(
        "开发助手",
        adapter,
        Some(Arc::new(Mutex::new(registry))),
        None,
        config,
        3,
    );

    // 验证工具已注册
    println!("✅ DevLogTool 已注册");
    println!("💡 Agent 现在可以使用 DevLog 工具记录开发决策和问题");
}

fn demo_4_persistence() {
    println!("\n{}", "=".repeat(60));
    println!("示例 4：持久化和恢复");
    println!("{}", "=".repeat(60));

    let session_id = "demo-session-004";

    // 第一次：创建工具并添加日志
    println!("\n📝 第一次会话：添加日志...");
    let tool1 = DevLogTool::new(
        session_id,
        "DemoAgent",
        PathBuf::from("."),
        PathBuf::from("memory/devlogs"),
    );

    tool1.run(json!({
        "action": "append",
        "category": "decision",
        "content": "决定使用微服务架构"
    }));
    tool1.run(json!({
        "action": "append",
        "category": "issue",
        "content": "服务间通信延迟高"
    }));

    println!("   ✅ 已添加 2 条日志");

    // 验证文件已创建
    let devlog_file = PathBuf::from(".")
        .join("memory/devlogs")
        .join(format!("devlog-{}.json", session_id));
    println!("   📁 日志文件: {}", devlog_file.display());
    println!("   ✅ 文件存在: {}", devlog_file.exists());

    // 第二次：创建新工具实例，应该自动加载已有日志
    println!("\n📖 第二次会话：自动加载已有日志...");
    let tool2 = DevLogTool::new(
        session_id,
        "DemoAgent",
        PathBuf::from("."),
        PathBuf::from("memory/devlogs"),
    );

    println!("   ✅ 已加载 {} 条日志", tool2.store.lock().unwrap().entries.len());

    // 生成摘要
    let response = tool2.run(json!({"action": "summary"}));
    println!("   {}", response.unwrap().text);

    // 继续添加日志
    println!("\n📝 继续添加日志...");
    tool2.run(json!({
        "action": "append",
        "category": "solution",
        "content": "使用 gRPC 替代 HTTP REST"
    }));

    println!("   ✅ 现在共有 {} 条日志", tool2.store.lock().unwrap().entries.len());
}

fn main() {
    dotenvy::dotenv().ok();

    demo_1_basic_operations();
    demo_2_filtering();
    demo_3_agent_integration();
    demo_4_persistence();

    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成");
    println!("{}", "=".repeat(60));
}