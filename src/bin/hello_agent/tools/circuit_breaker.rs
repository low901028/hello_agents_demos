use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::hello_agent::tools::response::{ToolResponse, ToolStatus};

/// 熔断器
pub struct CircuitBreaker {
    failure_threshold: u32,
    recovery_timeout: Duration,
    enabled: bool,
    failure_counts: HashMap<String, u32>,
    open_timestamps: HashMap<String, Instant>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout: u64, enabled: bool) -> Self {
        Self {
            failure_threshold,
            recovery_timeout: Duration::from_secs(recovery_timeout),
            enabled,
            failure_counts: HashMap::new(),
            open_timestamps: HashMap::new(),
        }
    }

    /// 检查工具是否被熔断
    pub fn is_open(&self, tool_name: &str) -> bool {
        if !self.enabled {
            return false;
        }

        if let Some(&open_time) = self.open_timestamps.get(tool_name) {
            if open_time.elapsed() > self.recovery_timeout {
                return false; // 超时自动恢复
            }
            return true;
        }

        false
    }

    /// 记录执行结果
    pub fn record_result(&mut self, tool_name: &str, response: &ToolResponse) {
        if !self.enabled {
            return;
        }

        if response.status == ToolStatus::Error {
            self.on_failure(tool_name);
        } else {
            self.on_success(tool_name);
        }
    }

    fn on_failure(&mut self, tool_name: &str) {
        let count = self.failure_counts.entry(tool_name.to_string()).or_insert(0);
        *count += 1;

        if *count >= self.failure_threshold {
            self.open_timestamps.insert(tool_name.to_string(), Instant::now());
            println!("🔴 Circuit Breaker: 工具 '{}' 已熔断（连续 {} 次失败）", tool_name, *count);
        }
    }

    fn on_success(&mut self, tool_name: &str) {
        self.failure_counts.insert(tool_name.to_string(), 0);
    }

    /// 手动开启熔断
    pub fn open(&mut self, tool_name: &str) {
        if !self.enabled {
            return;
        }
        self.open_timestamps.insert(tool_name.to_string(), Instant::now());
    }

    /// 关闭熔断
    pub fn close(&mut self, tool_name: &str) {
        self.failure_counts.insert(tool_name.to_string(), 0);
        self.open_timestamps.remove(tool_name);
    }

    /// 获取状态
    pub fn get_status(&self, tool_name: &str) -> serde_json::Value {
        if let Some(&open_time) = self.open_timestamps.get(tool_name) {
            let elapsed = open_time.elapsed();
            let recover_in = if elapsed > self.recovery_timeout {
                0
            } else {
                (self.recovery_timeout - elapsed).as_secs()
            };

            serde_json::json!({
                "state": "open",
                "failure_count": self.failure_counts.get(tool_name).copied().unwrap_or(0),
                "recover_in_seconds": recover_in,
            })
        } else {
            serde_json::json!({
                "state": "closed",
                "failure_count": self.failure_counts.get(tool_name).copied().unwrap_or(0),
            })
        }
    }
}