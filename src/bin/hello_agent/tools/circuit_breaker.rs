use crate::hello_agent::tools::response::{ToolResponse, ToolStatus};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    failure_threshold: usize,
    recovery_timeout: Duration,
    enabled: bool,
    failure_counts: HashMap<String, usize>,
    open_timestamps: HashMap<String, Instant>,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerStatus {
    pub state: String,
    pub failure_count: usize,
    pub recover_in_seconds: usize,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, recovery_timeout_secs: u64, enabled: bool) -> Self {
        CircuitBreaker {
            failure_threshold,
            recovery_timeout: Duration::from_secs(recovery_timeout_secs),
            enabled,
            failure_counts: HashMap::new(),
            open_timestamps: HashMap::new(),
        }
    }

    pub fn is_open(&mut self, tool_name: &str) -> bool {
        if !self.enabled {
            return false;
        }
        if let Some(&open_time) = self.open_timestamps.get(tool_name) {
            if open_time.elapsed() > self.recovery_timeout {
                self.close(tool_name);
                return false;
            }
            return true;
        }
        false
    }

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
        let count = self
            .failure_counts
            .entry(tool_name.to_string())
            .or_insert(0);
        *count += 1;
        if *count >= self.failure_threshold {
            self.open_timestamps
                .insert(tool_name.to_string(), Instant::now());
        }
    }

    fn on_success(&mut self, tool_name: &str) {
        self.failure_counts.insert(tool_name.to_string(), 0);
    }

    pub fn close(&mut self, tool_name: &str) {
        self.failure_counts.insert(tool_name.to_string(), 0);
        self.open_timestamps.remove(tool_name);
    }

    pub fn get_status(&self, tool_name: &str) -> CircuitBreakerStatus {
        if let Some(&open_time) = self.open_timestamps.get(tool_name) {
            let elapsed = open_time.elapsed();
            if elapsed > self.recovery_timeout {
                CircuitBreakerStatus {
                    state: "closed".into(),
                    failure_count: *self.failure_counts.get(tool_name).unwrap_or(&0),
                    recover_in_seconds: 0,
                }
            } else {
                CircuitBreakerStatus {
                    state: "open".into(),
                    failure_count: *self.failure_counts.get(tool_name).unwrap_or(&0),
                    recover_in_seconds: (self.recovery_timeout - elapsed).as_secs() as usize,
                }
            }
        } else {
            CircuitBreakerStatus {
                state: "closed".into(),
                failure_count: *self.failure_counts.get(tool_name).unwrap_or(&0),
                recover_in_seconds: 0,
            }
        }
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        CircuitBreaker::new(3, 300, true)
    }
}
