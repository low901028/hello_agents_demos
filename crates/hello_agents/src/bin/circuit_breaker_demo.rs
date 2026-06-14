// examples/circuit_breaker_demo.rs
// 熔断器机制使用示例

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use hello_agents::core::traits::tool::{Tool, ToolParameter};
use hello_agents::core::types::exceptions::HelloAgentException;
use hello_agents::tools::circuit_breaker::CircuitBreaker;
use hello_agents::tools::error::ToolErrorCode;
use hello_agents::tools::registry::ToolRegistry;
use hello_agents::tools::response::{ToolResponse, ToolStatus};
use hello_agents::tools::tool_base::ToolBase;

/// 不稳定的工具，用于演示熔断器
pub struct UnstableTool {
    base: ToolBase,
    call_count: Arc<Mutex<u32>>,
}

impl UnstableTool {
    pub fn new() -> Self {
        Self {
            base: ToolBase::new("UnstableTool", "一个不稳定的工具，用于测试熔断器", false),
            call_count: Arc::new(Mutex::new(0)),
        }
    }
}

impl Tool for UnstableTool {
    fn name(&self) -> &str {
        &self.base.name
    }
    fn description(&self) -> &str {
        &self.base.description
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter::new(
            "should_fail",
            "boolean",
            "是否应该失败",
            false,
            Some(Value::Bool(false)),
        )]
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        let should_fail = parameters
            .get("should_fail")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if should_fail {
            Ok(ToolResponse::error(
                ToolErrorCode::ExecutionError.as_str(),
                &format!("工具执行失败 (第 {} 次调用)", *count),
                None,
                None,
            ))
        } else {
            Ok(ToolResponse::success(
                format!("工具执行成功 (第 {} 次调用)", *count),
                Some(json!({"call_count": *count})),
                None,
                None,
            ))
        }
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
            call_count: Arc::clone(&self.call_count),
        })
    }
}

fn demo_auto_circuit_breaking() {
    println!("{}", "=".repeat(60));
    println!("示例 1: 自动熔断机制");
    println!("{}", "=".repeat(60));

    let breaker = CircuitBreaker::new(3, 5, true);
    let mut registry = ToolRegistry::new(Some(breaker));
    registry.register_tool(Box::new(UnstableTool::new()), false);

    println!("\n连续失败测试:");

    for i in 1..=3 {
        let resp = registry.execute_tool("UnstableTool", r#"{"should_fail": true}"#);
        let msg = if resp.status == ToolStatus::Error {
            resp.error_info.as_ref().map(|e| e.message.clone()).unwrap_or_default()
        } else {
            resp.text.clone()
        };
        println!("  调用 {}: {:?} - {}", i, resp.status, msg);
    }

    println!("\n第 4 次调用（应该被熔断）:");
    let resp = registry.execute_tool("UnstableTool", r#"{"should_fail": true}"#);
    println!("  状态: {:?}", resp.status);
    if let Some(err) = &resp.error_info {
        println!("  错误码: {}", err.code);
        println!("  消息: {}", err.message);
    }

    assert_eq!(resp.status, ToolStatus::Error);
    assert!(resp.error_info.is_some());
    assert_eq!(resp.error_info.as_ref().unwrap().code, ToolErrorCode::CircuitOpen.as_str());

    println!("\n✅ 自动熔断测试完成");
}

fn demo_auto_recovery() {
    println!("\n{}", "=".repeat(60));
    println!("示例 2: 自动恢复机制");
    println!("{}", "=".repeat(60));

    let breaker = CircuitBreaker::new(2, 2, true);
    let mut registry = ToolRegistry::new(Some(breaker));
    registry.register_tool(Box::new(UnstableTool::new()), false);

    println!("\n触发熔断:");
    for i in 1..=2 {
        let resp = registry.execute_tool("UnstableTool", r#"{"should_fail": true}"#);
        println!("  调用 {}: {:?}", i, resp.status);
    }

    let resp = registry.execute_tool("UnstableTool", r#"{"should_fail": false}"#);
    if let Some(err) = &resp.error_info {
        println!("\n熔断状态: {}", err.code);
        assert_eq!(err.code, ToolErrorCode::CircuitOpen.as_str());
    }

    println!("\n等待 2 秒自动恢复...");
    std::thread::sleep(Duration::from_secs(2).saturating_add(Duration::from_millis(100)));

    let resp = registry.execute_tool("UnstableTool", r#"{"should_fail": false}"#);
    println!("恢复后调用: {:?}", resp.status);
    assert_eq!(resp.status, ToolStatus::Success);

    println!("\n✅ 自动恢复测试完成");
}

fn demo_success_reset() {
    println!("\n{}", "=".repeat(60));
    println!("示例 3: 成功重置失败计数");
    println!("{}", "=".repeat(60));

    let breaker = CircuitBreaker::new(3, 60, true);
    let mut registry = ToolRegistry::new(Some(breaker));
    registry.register_tool(Box::new(UnstableTool::new()), false);

    println!("\n失败 -> 失败 -> 成功 -> 失败:");

    registry.execute_tool("UnstableTool", r#"{"should_fail": true}"#);
    println!("  调用 1: 失败 (计数: 1)");
    registry.execute_tool("UnstableTool", r#"{"should_fail": true}"#);
    println!("  调用 2: 失败 (计数: 2)");

    let resp = registry.execute_tool("UnstableTool", r#"{"should_fail": false}"#);
    println!("  调用 3: 成功 (计数: 0) ← 重置");
    assert_eq!(resp.status, ToolStatus::Success);

    registry.execute_tool("UnstableTool", r#"{"should_fail": true}"#);
    println!("  调用 4: 失败 (计数: 1)");
    let resp = registry.execute_tool("UnstableTool", r#"{"should_fail": true}"#);
    println!("  调用 5: 失败 (计数: 2)");

    assert_eq!(resp.status, ToolStatus::Error);
    assert_ne!(resp.error_info.as_ref().unwrap().code, ToolErrorCode::CircuitOpen.as_str());

    println!("\n✅ 成功重置测试完成");
}

fn demo_manual_control() {
    println!("\n{}", "=".repeat(60));
    println!("示例 4: 手动控制熔断器");
    println!("{}", "=".repeat(60));

    let breaker = CircuitBreaker::new(3, 60, true);
    let mut registry = ToolRegistry::new(Some(breaker));
    registry.register_tool(Box::new(UnstableTool::new()), false);

    // 手动开启熔断
    println!("\n手动开启熔断:");
    registry.circuit_breaker.lock().unwrap().open("UnstableTool");

    let resp = registry.execute_tool("UnstableTool", r#"{"should_fail": false}"#);
    println!("  状态: {:?}", resp.status);
    assert_eq!(resp.error_info.as_ref().unwrap().code, ToolErrorCode::CircuitOpen.as_str());

    // 手动关闭熔断
    println!("\n手动关闭熔断:");
    registry.circuit_breaker.lock().unwrap().close("UnstableTool");

    let resp = registry.execute_tool("UnstableTool", r#"{"should_fail": false}"#);
    println!("  状态: {:?}", resp.status);
    assert_eq!(resp.status, ToolStatus::Success);

    println!("\n✅ 手动控制测试完成");
}

fn main() {
    demo_auto_circuit_breaking();
    demo_auto_recovery();
    demo_success_reset();
    demo_manual_control();

    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成！");
    println!("{}", "=".repeat(60));
}