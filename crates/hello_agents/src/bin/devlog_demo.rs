// examples/devlog_demo.rs
// DevLogTool 使用示例（异步版）

use std::path::PathBuf;
use serde_json::json;
use hello_agents::core::traits::tool::Tool;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::tools::builtin::devlog::DevLogTool;

async fn demo_1_basic_operations() -> Result<(), HelloAgentError> {
    println!("{}", "=".repeat(60));
    println!("示例 1：DevLogTool 基本操作");
    println!("{}", "=".repeat(60));

    let tool = DevLogTool::new(
        "demo-session-001",
        "DemoAgent",
        &PathBuf::from("."),
        &PathBuf::from("memory/devlogs"),
    );

    println!("\n✅ DevLogTool 已创建");

    // 追加决策日志
    println!("\n📝 追加决策日志...");
    let response = tool.execute(json!({
        "action": "append",
        "category": "decision",
        "content": "选择使用 Redis 作为缓存层，因为需要支持分布式部署和高并发访问",
        "metadata": {
            "tags": ["architecture", "cache", "redis"],
            "step": 3,
            "related_tool": "WriteTool"
        }
    })).await?;
    println!("   {}", response.text);

    // 追加问题日志
    println!("\n📝 追加问题日志...");
    let response = tool.execute(json!({
        "action": "append",
        "category": "issue",
        "content": "API 响应时间超过 2 秒，影响用户体验",
        "metadata": {
            "tags": ["performance", "api"],
            "severity": "high"
        }
    })).await?;
    println!("   {}", response.text);

    // 追加解决方案日志
    println!("\n📝 追加解决方案日志...");
    let response = tool.execute(json!({
        "action": "append",
        "category": "solution",
        "content": "增加 Redis 缓存，缓存热点数据，减少数据库查询",
        "metadata": {
            "tags": ["performance", "cache"],
            "related_issue": "API 响应时间超过 2 秒"
        }
    })).await?;
    println!("   {}", response.text);

    // 生成摘要
    println!("\n📊 生成摘要...");
    let response = tool.execute(json!({"action": "summary"})).await?;
    println!("   {}", response.text);

    // 读取所有日志
    println!("\n📖 读取所有日志...");
    let response = tool.execute(json!({"action": "read"})).await?;
    println!("{}", response.text);

    Ok(())
}

async fn demo_2_filtering() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("示例 2：过滤查询");
    println!("{}", "=".repeat(60));

    let tool = DevLogTool::new(
        "demo-session-002",
        "DemoAgent",
        &PathBuf::from("."),
        &PathBuf::from("memory/devlogs"),
    );

    let logs = vec![
        json!({"action": "append", "category": "decision", "content": "使用 PostgreSQL 作为主数据库", "metadata": {"tags": ["database"]}}),
        json!({"action": "append", "category": "decision", "content": "使用 Redis 作为缓存", "metadata": {"tags": ["cache"]}}),
        json!({"action": "append", "category": "issue", "content": "数据库连接池耗尽", "metadata": {"tags": ["database", "performance"]}}),
        json!({"action": "append", "category": "solution", "content": "增加连接池大小到 50", "metadata": {"tags": ["database"]}}),
        json!({"action": "append", "category": "refactor", "content": "重构用户认证模块", "metadata": {"tags": ["auth", "security"]}}),
    ];

    for log in logs {
        tool.execute(log).await?;
    }

    println!("\n✅ 已添加 {} 条日志", 5);

    // 按类别过滤
    println!("\n🔍 只查看决策类日志...");
    let response = tool.execute(json!({
        "action": "read",
        "filter": {"category": "decision"}
    })).await?;
    println!("{}", response.text);

    // 按标签过滤
    println!("\n🔍 只查看数据库相关日志...");
    let response = tool.execute(json!({
        "action": "read",
        "filter": {"tags": ["database"]}
    })).await?;
    println!("{}", response.text);

    // 限制数量
    println!("\n🔍 只查看最近 2 条日志...");
    let response = tool.execute(json!({
        "action": "read",
        "filter": {"limit": 2}
    })).await?;
    println!("{}", response.text);

    Ok(())
}

async fn demo_3_agent_integration() {
    println!("\n{}", "=".repeat(60));
    println!("示例 3：Agent 集成 - 零配置使用");
    println!("{}", "=".repeat(60));

    // 此处仅展示概念，实际需配合 AgentRuntime 和 ToolRegistry
    println!("✅ DevLogTool 已注册");
    println!("💡 Agent 现在可以使用 DevLog 工具记录开发决策和问题");
}

async fn demo_4_persistence() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("示例 4：持久化和恢复");
    println!("{}", "=".repeat(60));

    let session_id = "demo-session-004";

    println!("\n📝 第一次会话：添加日志...");
    let tool1 = DevLogTool::new(
        session_id,
        "DemoAgent",
        &PathBuf::from("."),
        &PathBuf::from("memory/devlogs"),
    );

    tool1.execute(json!({
        "action": "append",
        "category": "decision",
        "content": "决定使用微服务架构"
    })).await?;
    tool1.execute(json!({
        "action": "append",
        "category": "issue",
        "content": "服务间通信延迟高"
    })).await?;

    println!("   ✅ 已添加 2 条日志");

    let devlog_file = PathBuf::from(".")
        .join("memory/devlogs")
        .join(format!("devlog-{}.json", session_id));
    println!("   📁 日志文件: {}", devlog_file.display());
    println!("   ✅ 文件存在: {}", devlog_file.exists());

    println!("\n📖 第二次会话：自动加载已有日志...");
    let tool2 = DevLogTool::new(
        session_id,
        "DemoAgent",
        &PathBuf::from("."),
        &PathBuf::from("memory/devlogs"),
    );

    // 获取已加载日志数量（假设 store 仍可访问）
    let entry_count = tool2.store.lock().unwrap().entries.len();
    println!("   ✅ 已加载 {} 条日志", entry_count);

    let response = tool2.execute(json!({"action": "summary"})).await?;
    println!("   {}", response.text);

    println!("\n📝 继续添加日志...");
    tool2.execute(json!({
        "action": "append",
        "category": "solution",
        "content": "使用 gRPC 替代 HTTP REST"
    })).await?;

    let entry_count = tool2.store.lock().unwrap().entries.len();
    println!("   ✅ 现在共有 {} 条日志", entry_count);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    demo_1_basic_operations().await?;
    demo_2_filtering().await?;
    demo_3_agent_integration().await;
    demo_4_persistence().await?;

    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成");
    println!("{}", "=".repeat(60));
    Ok(())
}