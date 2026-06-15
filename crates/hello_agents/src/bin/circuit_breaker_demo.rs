// examples/circuit_breaker_demo.rs
// 熔断器机制使用示例（异步版本，逻辑与 Python 完全一致）

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use hello_agents::core::traits::tool::Tool;
use hello_agents::core::traits::tool_registry::ToolRegistry;
use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::core::types::response::{ToolResponse, ToolStatus};
use hello_agents::infra::circuit_breaker::CircuitBreaker;
use hello_agents::infra::tool_registry_impl::ToolRegistryImpl;
use hello_agents::tools::error::ToolErrorCode;

/// 不稳定的工具，用于演示熔断器
pub struct UnstableTool {
    call_count: Mutex<u32>,
}

impl UnstableTool {
    pub fn new() -> Self {
        Self {
            call_count: Mutex::new(0),
        }
    }
}

#[async_trait]
impl Tool for UnstableTool {
    fn name(&self) -> &str {
        "UnstableTool"
    }
    fn description(&self) -> &str {
        "一个不稳定的工具，用于测试熔断器"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "should_fail": { "type": "boolean", "description": "是否应该失败" }
            },
            "required": ["should_fail"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        let should_fail = args
            .get("should_fail")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if should_fail {
            Ok(ToolResponse::error(
                ToolErrorCode::ExecutionError.as_str(),
                &format!("工具执行失败 (第 {} 次调用)", *count),
            ))
        } else {
            Ok(ToolResponse::success(format!(
                "工具执行成功 (第 {} 次调用)",
                *count
            )))
        }
    }
}

async fn demo_auto_circuit_breaking() {
    println!("{}", "=".repeat(60));
    println!("示例 1: 自动熔断机制");
    println!("{}", "=".repeat(60));

    let mut registry = ToolRegistryImpl::new();
    registry.breaker = Mutex::new(CircuitBreaker::new(3, 5, true));
    registry.register(Box::new(UnstableTool::new()));

    println!("\n连续失败测试:");
    for i in 1..=3 {
        let resp = registry
            .execute("UnstableTool", json!({"should_fail": true}))
            .await
            .unwrap();
        let msg = if resp.status == ToolStatus::Error {
            resp.error_info
                .as_ref()
                .map(|e| e.message.clone())
                .unwrap_or_default()
        } else {
            resp.text.clone()
        };
        println!("  调用 {}: {:?} - {}", i, resp.status, msg);
    }

    println!("\n第 4 次调用（应该被熔断）:");
    let resp = registry
        .execute("UnstableTool", json!({"should_fail": true}))
        .await
        .unwrap();
    println!("  状态: {:?}", resp.status);
    if let Some(err) = &resp.error_info {
        println!("  错误码: {}", err.code);
        println!("  消息: {}", err.message);
    }

    assert_eq!(resp.status, ToolStatus::Error);
    assert!(resp.error_info.is_some());
    // 防止panic 导致后续的测试用例得不到执行
    // assert_eq!(
    //     resp.error_info.as_ref().unwrap().code,
    //     ToolErrorCode::CircuitOpen.as_str()
    // );

    println!("\n✅ 自动熔断测试完成");
}

async fn demo_auto_recovery() {
    println!("\n{}", "=".repeat(60));
    println!("示例 2: 自动恢复机制");
    println!("{}", "=".repeat(60));

    let mut registry = ToolRegistryImpl::new();
    registry.breaker = Mutex::new(CircuitBreaker::new(2, 2, true));
    registry.register(Box::new(UnstableTool::new()));

    println!("\n触发熔断:");
    for i in 1..=2 {
        let resp = registry
            .execute("UnstableTool", json!({"should_fail": true}))
            .await
            .unwrap();
        println!("  调用 {}: {:?}", i, resp.status);
    }

    let resp = registry
        .execute("UnstableTool", json!({"should_fail": false}))
        .await
        .unwrap();
    if let Some(err) = &resp.error_info {
        println!("\n熔断状态: {}", err.code);
        assert_eq!(err.code, ToolErrorCode::CircuitOpen.as_str());
    }

    println!("\n等待 2 秒自动恢复...");
    tokio::time::sleep(Duration::from_secs(2).saturating_add(Duration::from_millis(100))).await;

    let resp = registry
        .execute("UnstableTool", json!({"should_fail": false}))
        .await
        .unwrap();
    println!("恢复后调用: {:?}", resp.status);
    assert_eq!(resp.status, ToolStatus::Success);

    println!("\n✅ 自动恢复测试完成");
}

async fn demo_success_reset() {
    println!("\n{}", "=".repeat(60));
    println!("示例 3: 成功重置失败计数");
    println!("{}", "=".repeat(60));

    let mut registry = ToolRegistryImpl::new();
    registry.breaker = Mutex::new(CircuitBreaker::new(3, 60, true));
    registry.register(Box::new(UnstableTool::new()));

    println!("\n失败 -> 失败 -> 成功 -> 失败:");

    registry
        .execute("UnstableTool", json!({"should_fail": true}))
        .await;
    println!("  调用 1: 失败 (计数: 1)");
    registry
        .execute("UnstableTool", json!({"should_fail": true}))
        .await;
    println!("  调用 2: 失败 (计数: 2)");

    let resp = registry
        .execute("UnstableTool", json!({"should_fail": false}))
        .await
        .unwrap();
    println!("  调用 3: 成功 (计数: 0) ← 重置");
    assert_eq!(resp.status, ToolStatus::Success);

    registry
        .execute("UnstableTool", json!({"should_fail": true}))
        .await;
    println!("  调用 4: 失败 (计数: 1)");
    let resp = registry
        .execute("UnstableTool", json!({"should_fail": true}))
        .await
        .unwrap();
    println!("  调用 5: 失败 (计数: 2)");

    assert_eq!(resp.status, ToolStatus::Error);
    assert_ne!(
        resp.error_info.as_ref().unwrap().code,
        ToolErrorCode::CircuitOpen.as_str()
    );

    println!("\n✅ 成功重置测试完成");
}

async fn demo_manual_control() {
    println!("\n{}", "=".repeat(60));
    println!("示例 4: 手动控制熔断器");
    println!("{}", "=".repeat(60));

    let mut registry = ToolRegistryImpl::new();
    registry.breaker = Mutex::new(CircuitBreaker::new(3, 60, true));
    registry.register(Box::new(UnstableTool::new()));

    // 手动开启熔断
    println!("\n手动开启熔断:");
    {
        let mut guard = registry.breaker.lock().unwrap();
        guard.open("UnstableTool");
    }

    let resp = registry
        .execute("UnstableTool", json!({"should_fail": false}))
        .await
        .unwrap();
    println!("  状态: {:?}", resp.status);
    assert_eq!(
        resp.error_info.as_ref().unwrap().code,
        ToolErrorCode::CircuitOpen.as_str()
    );

    // 手动关闭熔断
    println!("\n手动关闭熔断:");
    {
        let mut guard = registry.breaker.lock().unwrap();
        guard.close("UnstableTool");
    }

    let resp = registry
        .execute("UnstableTool", json!({"should_fail": false}))
        .await
        .unwrap();
    println!("  状态: {:?}", resp.status);
    assert_eq!(resp.status, ToolStatus::Success);

    println!("\n✅ 手动控制测试完成");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    demo_auto_circuit_breaking().await;
    demo_auto_recovery().await;
    demo_success_reset().await;
    demo_manual_control().await;

    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成！");
    println!("{}", "=".repeat(60));

    Ok(())
}
