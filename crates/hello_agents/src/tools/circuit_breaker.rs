//! circuit_breaker.rs
//! 熔断器机制 - 防止工具连续失败导致的死循环

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::tools::tool_response::{ToolResponse, ToolStatus};

/// 工具熔断器
///
/// 特性：
/// - 连续失败自动禁用工具
/// - 超时自动恢复
/// - 基于 ToolResponse 协议判断错误
///
/// 状态机：
/// Closed (正常) → Open (熔断) → Closed (恢复)
pub struct CircuitBreaker {
    /// 连续失败多少次后熔断
    failure_threshold: usize,
    /// 熔断后恢复时间（秒）
    recovery_timeout: u64,
    /// 是否启用熔断器
    enabled: bool,

    /// 每个工具的失败计数
    failure_counts: HashMap<String, usize>,
    /// 工具熔断开启的时间戳（绝对时间）
    open_timestamps: HashMap<String, SystemTime>,
}

impl CircuitBreaker {
    /// 创建熔断器
    ///
    /// # Arguments
    /// * `failure_threshold` - 连续失败多少次后熔断（默认 3）
    /// * `recovery_timeout` - 熔断后恢复时间（秒，默认 300）
    /// * `enabled` - 是否启用熔断器（默认 true）
    pub fn new(
        failure_threshold: usize,
        recovery_timeout: u64,
        enabled: bool,
    ) -> Self {
        Self {
            failure_threshold,
            recovery_timeout,
            enabled,
            failure_counts: HashMap::new(),
            open_timestamps: HashMap::new(),
        }
    }

    /// 检查工具是否被熔断
    ///
    /// # Arguments
    /// * `tool_name` - 工具名称
    ///
    /// # Returns
    /// * `true` - 工具被禁用
    /// * `false` - 工具可用
    pub fn is_open(&mut self, tool_name: &str) -> bool {
        if !self.enabled {
            return false;
        }

        // 检查是否在熔断列表中
        match self.open_timestamps.get(tool_name) {
            None => false,
            Some(open_time) => {
                // 检查是否已过恢复时间
                if let Ok(elapsed) = SystemTime::now().duration_since(*open_time) {
                    if elapsed.as_secs() >= self.recovery_timeout {
                        // 自动恢复
                        self.close(tool_name);
                        return false;
                    }
                }
                true
            }
        }
    }

    /// 记录工具执行结果
    ///
    /// # Arguments
    /// * `tool_name` - 工具名称
    /// * `response` - 工具响应对象
    pub fn record_result(&mut self, tool_name: &str, response: &ToolResponse) {
        if !self.enabled {
            return;
        }

        let is_error = response.status == ToolStatus::Error;

        if is_error {
            self.on_failure(tool_name);
        } else {
            self.on_success(tool_name);
        }
    }

    /// 处理失败
    fn on_failure(&mut self, tool_name: &str) {
        let count = self.failure_counts.entry(tool_name.to_string()).or_insert(0);
        *count += 1;

        if *count >= self.failure_threshold {
            self.open_timestamps
                .entry(tool_name.to_string())
                .or_insert(SystemTime::now());
            println!(
                "🔴 Circuit Breaker: 工具 '{}' 已熔断（连续 {} 次失败）",
                tool_name, *count
            );
        }
    }

    /// 处理成功
    fn on_success(&mut self, tool_name: &str) {
        self.failure_counts.insert(tool_name.to_string(), 0);
    }

    /// 手动开启熔断
    pub fn open(&mut self, tool_name: &str) {
        if !self.enabled {
            return;
        }

        self.open_timestamps
            .insert(tool_name.to_string(), SystemTime::now());
        println!("🔴 Circuit Breaker: 工具 '{}' 已手动熔断", tool_name);
    }

    /// 关闭熔断，恢复工具
    pub fn close(&mut self, tool_name: &str) {
        self.failure_counts.insert(tool_name.to_string(), 0);
        self.open_timestamps.remove(tool_name);
        println!("🟢 Circuit Breaker: 工具 '{}' 已恢复", tool_name);
    }

    /// 获取单个工具的熔断状态
    ///
    /// # Returns
    /// 状态字典，包含：
    /// - state: "open" | "closed"
    /// - failure_count: 失败次数
    /// - open_since: 熔断开始时间戳（秒，仅 open 状态）
    /// - recover_in_seconds: 剩余恢复时间（秒，仅 open 状态）
    pub fn get_status(&self, tool_name: &str) -> HashMap<String, serde_json::Value> {
        let mut status = HashMap::new();
        let failure_count = self.failure_counts.get(tool_name).copied().unwrap_or(0);

        if let Some(open_time) = self.open_timestamps.get(tool_name) {
            // 计算熔断开始时间戳（UNIX 秒）
            let open_since = open_time
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            let now = SystemTime::now();
            let time_since_open = now
                .duration_since(*open_time)
                .unwrap_or_default()
                .as_secs();
            let recover_in = if self.recovery_timeout > time_since_open {
                self.recovery_timeout - time_since_open
            } else {
                0
            };

            status.insert("state".to_string(), serde_json::Value::String("open".into()));
            status.insert(
                "failure_count".to_string(),
                serde_json::Value::Number(serde_json::Number::from(failure_count)),
            );
            status.insert(
                "open_since".to_string(),
                serde_json::Value::Number(serde_json::Number::from_f64(open_since).unwrap()),
            );
            status.insert(
                "recover_in_seconds".to_string(),
                serde_json::Value::Number(serde_json::Number::from(recover_in as u64)),
            );
        } else {
            status.insert(
                "state".to_string(),
                serde_json::Value::String("closed".into()),
            );
            status.insert(
                "failure_count".to_string(),
                serde_json::Value::Number(serde_json::Number::from(failure_count)),
            );
        }

        status
    }

    /// 获取所有工具的熔断状态
    pub fn get_all_status(&self) -> HashMap<String, HashMap<String, serde_json::Value>> {
        let mut all_tools: Vec<&String> = self
            .failure_counts
            .keys()
            .chain(self.open_timestamps.keys())
            .collect();
        all_tools.sort();
        all_tools.dedup();

        all_tools
            .into_iter()
            .map(|name| (name.clone(), self.get_status(name)))
            .collect()
    }
}

// ---------- 测试 ----------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::tool_response::ToolResponse;
    use std::thread::sleep;
    use std::time::Duration;

    fn new_cb() -> CircuitBreaker {
        CircuitBreaker::new(3, 2, true) // 阈值3，恢复时间2秒（便于测试）
    }

    fn success_response() -> ToolResponse {
        ToolResponse::success("ok", None, None, None)
    }

    fn error_response() -> ToolResponse {
        ToolResponse::error("ERR", "failure", None, None)
    }

    #[test]
    fn test_new_breaker_state() {
        let mut cb = new_cb();
        assert!(!cb.is_open("tool_a"));
        let status = cb.get_status("tool_a");
        assert_eq!(status["state"], "closed");
        assert_eq!(status["failure_count"], 0);
    }

    #[test]
    fn test_failure_count_and_open() {
        let mut cb = new_cb();
        // 两次失败不应触发熔断
        cb.record_result("tool_a", &error_response());
        cb.record_result("tool_a", &error_response());
        assert!(!cb.is_open("tool_a"));
        assert_eq!(cb.failure_counts.get("tool_a"), Some(&2));

        // 第三次失败触发熔断
        cb.record_result("tool_a", &error_response());
        assert!(cb.is_open("tool_a"));
        let status = cb.get_status("tool_a");
        assert_eq!(status["state"], "open");
        assert_eq!(status["failure_count"], 3);
        assert!(status.contains_key("open_since"));
        assert!(status["recover_in_seconds"].as_u64().unwrap() <= 2);
    }

    #[test]
    fn test_success_resets_count() {
        let mut cb = new_cb();
        cb.record_result("tool_a", &error_response());
        cb.record_result("tool_a", &error_response());
        // 一次成功应重置计数
        cb.record_result("tool_a", &success_response());
        assert_eq!(cb.failure_counts.get("tool_a"), Some(&0));
        assert!(!cb.is_open("tool_a"));
    }

    #[test]
    fn test_auto_recovery() {
        let mut cb = CircuitBreaker::new(1, 2, true); // 1次失败即熔断，2秒恢复
        cb.record_result("tool_a", &error_response());
        assert!(cb.is_open("tool_a"));

        // 等待超过恢复时间
        sleep(Duration::from_secs(3));
        assert!(!cb.is_open("tool_a")); // 应自动恢复
    }

    #[test]
    fn test_manual_open_close() {
        let mut cb = new_cb();
        assert!(!cb.is_open("tool_a"));
        cb.open("tool_a");
        assert!(cb.is_open("tool_a"));
        cb.close("tool_a");
        assert!(!cb.is_open("tool_a"));
    }

    #[test]
    fn test_disabled() {
        let mut cb = CircuitBreaker::new(1, 2, false);
        // 即使连续失败也不会熔断
        cb.record_result("tool_a", &error_response());
        assert!(!cb.is_open("tool_a"));
        cb.open("tool_a");
        assert!(!cb.is_open("tool_a"));
    }

    #[test]
    fn test_get_all_status() {
        let mut cb = new_cb();
        cb.record_result("tool_a", &error_response());
        cb.record_result("tool_b", &success_response());
        let all = cb.get_all_status();
        assert_eq!(all.len(), 2);
        assert_eq!(all["tool_a"]["state"], "closed");
        assert_eq!(all["tool_a"]["failure_count"], 1);
        assert_eq!(all["tool_b"]["state"], "closed");
        assert_eq!(all["tool_b"]["failure_count"], 0);
    }

    #[test]
    fn test_partial_status_is_not_error() {
        let mut cb = new_cb();
        let partial = ToolResponse::partial("partial result", None, None, None);
        cb.record_result("tool_a", &partial);
        // 部分成功不应算作失败
        assert_eq!(cb.failure_counts.get("tool_a"), Some(&0));
        assert!(!cb.is_open("tool_a"));
    }
}