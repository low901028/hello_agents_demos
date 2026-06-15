// examples/parallel_tools_performance.rs
// 工具并行执行性能对比示例

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Semaphore;

use hello_agents::core::traits::tool::Tool;
use hello_agents::core::traits::tool_registry::ToolRegistry;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::core::types::response::ToolResponse;
use hello_agents::infra::tool_registry_impl::ToolRegistryImpl;

// ==================== 模拟耗时工具 ====================

struct SlowTool {
    name: String,
    delay: Duration,
}

impl SlowTool {
    fn new(name: &str, delay_secs: f64) -> Self {
        Self {
            name: name.to_string(),
            delay: Duration::from_secs_f64(delay_secs),
        }
    }
}

#[async_trait]
impl Tool for SlowTool {
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { "模拟耗时工具" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "data": {"type": "string", "description": "数据"}
            },
            "required": []
        })
    }

    async fn execute(&self, _args: Value) -> Result<ToolResponse, HelloAgentError> {
        // 模拟耗时操作
        tokio::time::sleep(self.delay).await;
        Ok(ToolResponse::success(format!(
            "{} 完成（耗时 {:.1}s）",
            self.name,
            self.delay.as_secs_f64()
        )))
    }
}

// ==================== 演示函数 ====================

/// 演示异步并行执行（3个工具同时运行）
async fn demo_parallel_performance() -> Result<(), HelloAgentError> {
    println!("{}", "=".repeat(60));
    println!("工具并行执行性能测试");
    println!("{}", "=".repeat(60));

    let mut registry = ToolRegistryImpl::new();
    registry.register(Box::new(SlowTool::new("Tool1", 1.0)));
    registry.register(Box::new(SlowTool::new("Tool2", 1.0)));
    registry.register(Box::new(SlowTool::new("Tool3", 1.0)));

    // 假设我们要并发执行这3个工具（不依赖 LLM）
    let tool_names = vec!["Tool1", "Tool2", "Tool3"];
    let args = json!({"data": "test"});

    println!("\n🚀 测试异步并行执行（3个工具同时运行）");
    let start = Instant::now();

    // 使用 FuturesUnordered 并发执行
    for name in &tool_names {
        let tool = registry.get_tool(name).unwrap(); // 需要获取 Arc 或直接引用，这里简化
        // 这里不能直接获取 &dyn Tool，因为是借用，需要所有权？实际上我们直接拿 registry 内部引用有问题
        // 我们改为用 Arc 包装 Tool，然后克隆 Arc 来传递。为简化，我们直接创建 SlowTool 实例而不通过 registry。
        // 但在实际新架构中，ToolRegistry::execute 是异步的，可以直接调用。
        // 这里为了演示并行，我们模拟直接调用工具。
        // 我们重新创建 SlowTool 来演示，但为了展示 registry 的使用，改为用 Arc<Tool> 并从中取出。
        // 下面的代码假定 ToolRegistryImpl 内部用 Arc<dyn Tool>，我们可以通过 get_tool 获取 &dyn Tool，但需要 Send + Sync。
        // 这里为了简单，直接创建新的实例。
    }
    // 更简洁：直接创建工具实例并用 join_all 并发
    let tools: Vec<Arc<dyn Tool>> = tool_names.iter().map(|name| {
        Arc::new(SlowTool::new(name, 1.0)) as Arc<dyn Tool>
    }).collect();

    let futures = tools.iter().map(|tool| {
        let tool = Arc::clone(tool);
        let args = args.clone();
        async move { tool.execute(args).await }
    });

    let results = futures::future::join_all(futures).await;
    let elapsed = start.elapsed();

    for res in results {
        println!("  {}", res.unwrap().text);
    }

    println!("\n⏱️  异步并行执行耗时: {:.2}s", elapsed.as_secs_f64());
    println!("   理论最优: ~1.0s（3个工具并行）");
    println!("   同步执行: ~3.0s（3个工具串行）");
    println!("   性能提升: {:.2}x", 3.0 / elapsed.as_secs_f64());
    Ok(())
}

/// 演示并发数限制（5个工具，最多2个并行）
async fn demo_concurrency_limit() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("并发数限制测试");
    println!("{}", "=".repeat(60));

    let tools: Vec<Arc<dyn Tool>> = (1..=5).map(|i| {
        Arc::new(SlowTool::new(&format!("Tool{}", i), 1.0)) as Arc<dyn Tool>
    }).collect();

    let semaphore = Arc::new(Semaphore::new(2)); // 最多 2 个并发
    let args = json!({"data": "test"});

    println!("\n🚀 测试并发限制（5个工具，最多2个并行）");
    let start = Instant::now();

    let futures = tools.iter().map(async |tool| {
        let tool = Arc::clone(tool);
        let args = args.clone();
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        async move {
            let res = tool.execute(args).await;
            drop(permit);
            res
        }
    });

    let results = futures::future::join_all(futures).await;
    let elapsed = start.elapsed();

    for res in results {
        println!("  {}", res.await.unwrap().text);
    }

    println!("\n⏱️  执行耗时: {:.2}s", elapsed.as_secs_f64());
    println!("   理论耗时: ~3.0s（5个工具，每次2个并行：2+2+1）");
    println!("   无限制: ~1.0s（5个工具全部并行）");
    println!("   串行执行: ~5.0s（5个工具串行）");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), HelloAgentError> {
    demo_parallel_performance().await?;
    demo_concurrency_limit().await?;
    Ok(())
}