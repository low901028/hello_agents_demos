// ============================================================
// src/infra/circuit_breaker.rs
// 熔断器实现 – 防止工具连续失败
// ============================================================

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// 连续失败多少次后熔断
    failure_threshold: usize,
    /// 熔断后自动恢复的超时（秒）
    recovery_timeout: u64,
    /// 是否启用
    enabled: bool,
    /// 每个工具的失败计数
    failure_counts: HashMap<String, usize>,
    /// 每个工具熔断打开的时间
    open_timestamps: HashMap<String, SystemTime>,
}

impl CircuitBreaker {
    /// 创建新的熔断器
    ///
    /// * `failure_threshold` - 连续失败多少次后熔断
    /// * `recovery_timeout` - 熔断后自动恢复的超时（秒）
    /// * `enabled` - 是否启用
    pub fn new(failure_threshold: usize, recovery_timeout: u64, enabled: bool) -> Self {
        Self {
            failure_threshold,
            recovery_timeout,
            enabled,
            failure_counts: HashMap::new(),
            open_timestamps: HashMap::new(),
        }
    }

    /// 检查工具是否已熔断。
    /// 如果已过恢复期，则自动恢复（返回 false）。
    pub fn is_open(&mut self, tool_name: &str) -> bool {
        if !self.enabled {
            return false;
        }

        if let Some(open_time) = self.open_timestamps.get(tool_name) {
            if let Ok(elapsed) = SystemTime::now().duration_since(*open_time) {
                if elapsed.as_secs() >= self.recovery_timeout {
                    // 超时自动恢复
                    self.failure_counts.remove(tool_name);
                    self.open_timestamps.remove(tool_name);
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    /// 记录工具执行成功，重置失败计数并关闭熔断（如果打开）。
    pub fn on_success(&mut self, tool_name: &str) {
        if !self.enabled {
            return;
        }
        self.failure_counts.insert(tool_name.to_string(), 0);
        self.open_timestamps.remove(tool_name);
    }

    /// 记录工具执行失败，累加计数，达到阈值则打开熔断。
    pub fn on_failure(&mut self, tool_name: &str) {
        if !self.enabled {
            return;
        }
        let count = self
            .failure_counts
            .entry(tool_name.to_string())
            .or_insert(0);
        *count += 1;
        if *count >= self.failure_threshold {
            self.open_timestamps
                .entry(tool_name.to_string())
                .or_insert_with(SystemTime::now);
        }
    }

    /// 手动打开熔断。
    pub fn open(&mut self, tool_name: &str) {
        if !self.enabled {
            return;
        }
        self.open_timestamps
            .insert(tool_name.to_string(), SystemTime::now());
    }

    /// 手动关闭熔断。
    pub fn close(&mut self, tool_name: &str) {
        if !self.enabled {
            return;
        }
        self.failure_counts.remove(tool_name);
        self.open_timestamps.remove(tool_name);
    }
}
