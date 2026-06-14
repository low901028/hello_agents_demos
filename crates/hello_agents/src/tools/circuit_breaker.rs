use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::tools::response::{ToolResponse, ToolStatus};

pub struct CircuitBreaker {
    failure_threshold: usize,
    recovery_timeout: u64,
    enabled: bool,
    failure_counts: HashMap<String, usize>,
    open_timestamps: HashMap<String, SystemTime>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, recovery_timeout: u64, enabled: bool) -> Self {
        Self { failure_threshold, recovery_timeout, enabled, failure_counts: HashMap::new(), open_timestamps: HashMap::new() }
    }

    pub fn is_open(&mut self, tool_name: &str) -> bool {
        if !self.enabled { return false; }
        match self.open_timestamps.get(tool_name) {
            None => false,
            Some(open_time) => {
                if let Ok(elapsed) = SystemTime::now().duration_since(*open_time) {
                    if elapsed.as_secs() >= self.recovery_timeout { self.close(tool_name); return false; }
                }
                true
            }
        }
    }

    pub fn record_result(&mut self, tool_name: &str, response: &ToolResponse) {
        if !self.enabled { return; }
        if response.status == ToolStatus::Error { self.on_failure(tool_name); } else { self.on_success(tool_name); }
    }

    fn on_failure(&mut self, tool_name: &str) {
        let count = self.failure_counts.entry(tool_name.to_string()).or_insert(0);
        *count += 1;
        if *count >= self.failure_threshold {
            self.open_timestamps.entry(tool_name.to_string()).or_insert(SystemTime::now());
            println!("🔴 Circuit Breaker: 工具 '{}' 已熔断（连续 {} 次失败）", tool_name, *count);
        }
    }

    fn on_success(&mut self, tool_name: &str) { self.failure_counts.insert(tool_name.to_string(), 0); }

    pub fn open(&mut self, tool_name: &str) {
        if !self.enabled { return; }
        self.open_timestamps.insert(tool_name.to_string(), SystemTime::now());
        println!("🔴 Circuit Breaker: 工具 '{}' 已手动熔断", tool_name);
    }

    pub fn close(&mut self, tool_name: &str) {
        self.failure_counts.insert(tool_name.to_string(), 0);
        self.open_timestamps.remove(tool_name);
        println!("🟢 Circuit Breaker: 工具 '{}' 已恢复", tool_name);
    }

    pub fn get_status(&self, tool_name: &str) -> HashMap<String, serde_json::Value> {
        let mut status = HashMap::new();
        let fc = self.failure_counts.get(tool_name).copied().unwrap_or(0);
        if let Some(ot) = self.open_timestamps.get(tool_name) {
            let open_since = ot.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64();
            let time_since_open = SystemTime::now().duration_since(*ot).unwrap_or_default().as_secs();
            let recover_in = if self.recovery_timeout > time_since_open { self.recovery_timeout - time_since_open } else { 0 };
            status.insert("state".into(), serde_json::Value::String("open".into()));
            status.insert("failure_count".into(), serde_json::Value::Number(serde_json::Number::from(fc)));
            status.insert("open_since".into(), serde_json::Value::Number(serde_json::Number::from_f64(open_since).unwrap()));
            status.insert("recover_in_seconds".into(), serde_json::Value::Number(serde_json::Number::from(recover_in as u64)));
        } else {
            status.insert("state".into(), serde_json::Value::String("closed".into()));
            status.insert("failure_count".into(), serde_json::Value::Number(serde_json::Number::from(fc)));
        }
        status
    }
}