// examples/observability_demo.rs
// 可观测性使用示例（修复锁毒化、异步版本）

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use regex::Regex;
use serde_json::{json, Value};
use uuid::Uuid;

use hello_agents::core::types::exceptions::HelloAgentError;
use hello_agents::tools::error::ToolErrorCode;

/// 双格式 Trace 记录器（异步兼容，同步 I/O 安全）
pub struct TraceLogger {
    session_id: String,
    output_dir: PathBuf,
    sanitize: bool,
    events: Mutex<Vec<Value>>,
}

impl TraceLogger {
    pub fn new(output_dir: &str, sanitize: bool) -> Self {
        let dir = PathBuf::from(output_dir);
        fs::create_dir_all(&dir).expect("无法创建 Trace 输出目录");
        let session_id = format!(
            "s-{}-{}",
            Local::now().format("%Y%m%d-%H%M%S"),
            &Uuid::new_v4().to_string()[..4]
        );
        Self {
            session_id,
            output_dir: dir,
            sanitize,
            events: Mutex::new(Vec::new()),
        }
    }

    /// 记录事件（锁内无 I/O，防止毒化）
    pub fn log_event(&self, event_type: &str, payload: Value, step: Option<usize>) {
        // 1. 构造事件（不在锁内）
        let mut event = json!({
            "ts": Local::now().to_rfc3339(),
            "session_id": self.session_id,
            "event": event_type,
            "payload": payload,
        });
        if let Some(s) = step {
            event["step"] = json!(s);
        }
        if self.sanitize {
            event = Self::sanitize_event(event);
        }

        // 2. 安全地追加到内存列表（锁内只做 push，不 panic）
        {
            let mut events = self.events.lock().expect("TraceLogger events lock poisoned");
            events.push(event.clone());
        } // 锁在此处释放

        // 3. 写入 JSONL 文件（锁外，忽略错误）
        let jsonl_path = self.output_dir.join(format!("trace-{}.jsonl", self.session_id));
        let line = serde_json::to_string(&event).unwrap_or_default();
        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&jsonl_path)
        {
            use std::io::Write;
            let _ = writeln!(file, "{}", line);
        }
    }

    /// 递归脱敏
    fn sanitize_event(mut event: Value) -> Value {
        if let Some(payload) = event.get_mut("payload") {
            *payload = Self::sanitize_value(payload.clone());
        }
        event
    }

    fn sanitize_value(value: Value) -> Value {
        let re_sk = Regex::new(r"sk-[a-zA-Z0-9]+").unwrap();
        let re_bearer = Regex::new(r"Bearer\s+[a-zA-Z0-9_\-]+").unwrap();
        let re_path = Regex::new(r"(/Users/|/home/|C:\\Users\\)[^/\\]+").unwrap();

        match value {
            Value::String(s) => {
                let s = re_sk.replace_all(&s, "sk-***").to_string();
                let s = re_bearer.replace_all(&s, "Bearer ***").to_string();
                let s = re_path.replace_all(&s, "${1}***").to_string();
                Value::String(s)
            }
            Value::Object(map) => {
                let new_map = map
                    .into_iter()
                    .map(|(k, v)| (k, Self::sanitize_value(v)))
                    .collect();
                Value::Object(new_map)
            }
            Value::Array(arr) => {
                Value::Array(arr.into_iter().map(Self::sanitize_value).collect())
            }
            other => other,
        }
    }

    /// 完成并生成统计报告（处理锁毒化）
    pub fn finalize(&self) -> Value {
        // 安全获取锁，即使毒化也能恢复数据
        let events = self.events.lock().unwrap_or_else(|poisoned| {
            eprintln!("⚠️ events lock is poisoned, recovering with existing data");
            poisoned.into_inner()
        });

        let events_clone = events.clone();
        drop(events); // 立即释放锁

        let stats = Self::compute_stats(&events_clone);

        // 写入 HTML（忽略错误）
        let html_path = self.output_dir.join(format!("trace-{}.html", self.session_id));
        let html = format!(
            "<html><body><pre>{}</pre></body></html>",
            serde_json::to_string_pretty(&stats).unwrap()
        );
        if let Err(e) = fs::write(&html_path, html) {
            eprintln!("⚠️ 写入 HTML 文件失败: {}", e);
        } else {
            println!("✅ HTML 报告已生成: {}", html_path.display());
        }

        stats
    }

    fn compute_stats(events: &[Value]) -> Value {
        let mut total_steps = 0;
        let mut total_tokens = 0;
        let mut total_cost = 0.0;
        let mut tool_calls: HashMap<String, usize> = HashMap::new();
        let mut errors: Vec<Value> = Vec::new();
        let mut session_start: Option<f64> = None;
        let mut session_end: Option<f64> = None;

        for event in events {
            let event_type = event["event"].as_str().unwrap_or("");
            if let Some(step) = event.get("step").and_then(|s| s.as_u64()) {
                total_steps = total_steps.max(step as usize);
            }
            match event_type {
                "session_start" => {
                    if let Some(ts) = event["ts"].as_str() {
                        session_start = chrono::DateTime::parse_from_rfc3339(ts)
                            .ok()
                            .map(|dt| dt.timestamp_millis() as f64 / 1000.0);
                    }
                }
                "session_end" => {
                    if let Some(ts) = event["ts"].as_str() {
                        session_end = chrono::DateTime::parse_from_rfc3339(ts)
                            .ok()
                            .map(|dt| dt.timestamp_millis() as f64 / 1000.0);
                    }
                }
                "model_output" => {
                    if let Some(usage) = event["payload"].get("usage") {
                        total_tokens += usage
                            .get("total_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        total_cost += usage.get("cost").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    }
                }
                "tool_call" => {
                    let tool_name = event["payload"]["tool_name"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string();
                    *tool_calls.entry(tool_name).or_insert(0) += 1;
                }
                "error" => {
                    errors.push(json!({
                        "step": event.get("step"),
                        "type": event["payload"].get("error_type"),
                        "message": event["payload"].get("message"),
                    }));
                }
                _ => {}
            }
        }

        let duration = match (session_start, session_end) {
            (Some(start), Some(end)) => (end - start).max(0.0),
            _ => 0.0,
        };

        json!({
            "total_steps": total_steps,
            "total_tokens": total_tokens,
            "total_cost": total_cost,
            "tool_calls": tool_calls,
            "errors": errors,
            "duration_seconds": duration,
        })
    }
}

// ==================== 演示函数 ====================

fn demo_basic_logging() -> Result<(), HelloAgentError> {
    println!("{}", "=".repeat(60));
    println!("示例 1: 基本事件记录");
    println!("{}", "=".repeat(60));

    let output_dir = "memory/traces/demo";
    let logger = TraceLogger::new(output_dir, true);

    println!("\n会话 ID: {}", logger.session_id);

    logger.log_event(
        "session_start",
        json!({
            "agent_name": "DemoAgent",
            "llm_model": "gpt-4",
            "timestamp": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64()
        }),
        None,
    );

    logger.log_event(
        "tool_call",
        json!({
            "tool_name": "Calculator",
            "parameters": {"expression": "2 + 3"}
        }),
        Some(1),
    );

    logger.log_event(
        "tool_result",
        json!({
            "tool_name": "Calculator",
            "status": "success",
            "result": "5"
        }),
        Some(1),
    );

    logger.log_event(
        "llm_response",
        json!({
            "content": "计算结果是 5",
            "tokens": 10
        }),
        Some(1),
    );

    logger.log_event(
        "session_end",
        json!({
            "final_answer": "计算完成",
            "total_steps": 1
        }),
        None,
    );

    let stats = logger.finalize();

    let jsonl_path = format!("{}/trace-{}.jsonl", output_dir, logger.session_id);
    let html_path = format!("{}/trace-{}.html", output_dir, logger.session_id);
    assert!(std::path::Path::new(&jsonl_path).exists());
    assert!(std::path::Path::new(&html_path).exists());

    println!("\n✅ JSONL 文件: trace-{}.jsonl", logger.session_id);
    println!("✅ HTML 文件: trace-{}.html", logger.session_id);
    println!("✅ 事件数量: {}", stats["total_steps"]); // 简化，实际事件数可单独统计
    Ok(())
}

fn demo_sanitization() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("示例 2: 敏感信息脱敏");
    println!("{}", "=".repeat(60));

    let output_dir = "memory/traces/demo";
    let logger = TraceLogger::new(output_dir, true);

    logger.log_event(
        "config_loaded",
        json!({
            "api_key": "sk-1234567890abcdef",
            "openai_api_key": "sk-abcdefghijklmnop",
            "file_path": "C:/Users/admin/project/config.py",
            "database_url": "postgresql://user:pass@localhost/db"
        }),
        None,
    );

    // 获取脱敏后的事件
    let events = logger.events.lock().unwrap();
    let payload = &events[0]["payload"];

    println!("\n脱敏后的数据:");
    println!("  api_key: {}", payload.get("api_key").unwrap());
    println!("  openai_api_key: {}", payload.get("openai_api_key").unwrap());
    println!("  file_path: {}", payload.get("file_path").unwrap());

    let payload_str = payload.to_string();
    assert!(!payload_str.contains("sk-1234567890abcdef"));
    assert!(!payload_str.contains("sk-abcdefghijklmnop"));
    assert!(payload_str.contains("sk-***"));
    assert!(payload_str.contains("/Users/***/") || payload_str.contains("C:/Users/***/"));

    drop(events);
    logger.finalize();
    println!("\n✅ 敏感信息脱敏测试完成");
    Ok(())
}

fn demo_error_tracking() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("示例 3: 错误追踪");
    println!("{}", "=".repeat(60));

    let output_dir = "memory/traces/demo";
    let logger = TraceLogger::new(output_dir, false);

    logger.log_event("session_start", json!({"agent_name": "ErrorDemo"}), None);
    logger.log_event("tool_call", json!({"tool_name": "Read"}), Some(1));
    logger.log_event(
        "tool_result",
        json!({"tool_name": "Read", "status": "success"}),
        Some(1),
    );
    logger.log_event("tool_call", json!({"tool_name": "Write"}), Some(2));
    logger.log_event(
        "tool_result",
        json!({
            "tool_name": "Write",
            "status": "error",
            "error_code": ToolErrorCode::PermissionDenied.as_str(),
            "error_message": "没有写入权限"
        }),
        Some(2),
    );
    logger.log_event(
        "circuit_breaker",
        json!({
            "tool_name": "Write",
            "action": "opened",
            "reason": "连续失败 3 次"
        }),
        Some(3),
    );
    logger.log_event(
        "session_end",
        json!({"status": "error", "error": "工具执行失败"}),
        None,
    );

    logger.finalize();
    println!("\n✅ 错误追踪测试完成");
    Ok(())
}

fn demo_statistics() -> Result<(), HelloAgentError> {
    println!("\n{}", "=".repeat(60));
    println!("示例 4: 统计信息");
    println!("{}", "=".repeat(60));

    let output_dir = "memory/traces/demo";
    let logger = TraceLogger::new(output_dir, false);

    logger.log_event("session_start", json!({"agent_name": "StatsDemo"}), None);
    for step in 1..=5 {
        logger.log_event(
            "tool_call",
            json!({"tool_name": format!("Tool{}", step)}),
            Some(step),
        );
        logger.log_event(
            "tool_result",
            json!({"tool_name": format!("Tool{}", step), "status": "success"}),
            Some(step),
        );
        logger.log_event(
            "model_output",
            json!({
                "usage": {
                    "total_tokens": 50 + step * 10,
                    "cost": 0.001 * step as f64
                }
            }),
            Some(step),
        );
    }
    logger.log_event("session_end", json!({"total_steps": 5}), None);

    let stats = logger.finalize();

    println!("\n统计信息:");
    println!("  总步数: {}", stats["total_steps"]);
    println!(
        "  工具调用次数: {}",
        stats["tool_calls"].as_object().map_or(0, |m| m.len())
    );
    println!("  总 Tokens: {}", stats["total_tokens"]);
    println!("  总成本: ${:.4}", stats["total_cost"].as_f64().unwrap_or(0.0));
    println!(
        "  错误数: {}",
        stats["errors"].as_array().map_or(0, |a| a.len())
    );
    println!(
        "  会话时长: {:.2}s",
        stats["duration_seconds"].as_f64().unwrap_or(0.0)
    );

    println!("\n✅ 统计信息测试完成");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    demo_basic_logging()?;
    demo_sanitization()?;
    demo_error_tracking()?;
    demo_statistics()?;
    println!("\n{}", "=".repeat(60));
    println!("✅ 所有示例运行完成！");
    println!("{}", "=".repeat(60));
    Ok(())
}