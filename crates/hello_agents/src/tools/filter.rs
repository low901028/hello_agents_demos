//! filter.rs
//! 工具响应协议
//!
//! 标准化的工具响应格式，提供结构化的状态、数据和错误信息。
//!
use std::collections::HashSet;

pub trait ToolFilter: Send + Sync {
    fn filter(&self, all_tools: &[String]) -> Vec<String>;
    fn is_allowed(&self, tool_name: &str) -> bool;
}

const READONLY_TOOLS: &[&str] = &[
    "Read", "ReadTool", "LS", "LSTool",
    "Glob", "GlobTool", "Grep", "GrepTool",
    "Skill", "SkillTool",
];

pub struct ReadOnlyFilter {
    allowed: HashSet<String>,
}

impl ReadOnlyFilter {
    pub fn new(additional_allowed: Option<Vec<String>>) -> Self {
        let mut allowed: HashSet<String> = READONLY_TOOLS.iter().map(|s| s.to_string()).collect();
        if let Some(extra) = additional_allowed {
            allowed.extend(extra);
        }
        Self { allowed }
    }
}

impl ToolFilter for ReadOnlyFilter {
    fn filter(&self, all_tools: &[String]) -> Vec<String> {
        all_tools.iter().filter(|name| self.is_allowed(name)).cloned().collect()
    }
    fn is_allowed(&self, tool_name: &str) -> bool {
        self.allowed.contains(tool_name)
    }
}

const DENIED_TOOLS: &[&str] = &[
    "Bash", "BashTool", "Terminal", "TerminalTool", "Execute", "ExecuteTool",
];

pub struct FullAccessFilter {
    denied: HashSet<String>,
}

impl FullAccessFilter {
    pub fn new(additional_denied: Option<Vec<String>>) -> Self {
        let mut denied: HashSet<String> = DENIED_TOOLS.iter().map(|s| s.to_string()).collect();
        if let Some(extra) = additional_denied {
            denied.extend(extra);
        }
        Self { denied }
    }
}

impl ToolFilter for FullAccessFilter {
    fn filter(&self, all_tools: &[String]) -> Vec<String> {
        all_tools.iter().filter(|name| self.is_allowed(name)).cloned().collect()
    }
    fn is_allowed(&self, tool_name: &str) -> bool {
        !self.denied.contains(tool_name)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Whitelist,
    Blacklist,
}

pub struct CustomFilter {
    allowed: HashSet<String>,
    denied: HashSet<String>,
    mode: FilterMode,
}

impl CustomFilter {
    pub fn new(
        allowed: Option<Vec<String>>,
        denied: Option<Vec<String>>,
        mode: FilterMode,
    ) -> Self {
        Self {
            allowed: allowed.map(|v| v.into_iter().collect()).unwrap_or_default(),
            denied: denied.map(|v| v.into_iter().collect()).unwrap_or_default(),
            mode,
        }
    }
}

impl ToolFilter for CustomFilter {
    fn filter(&self, all_tools: &[String]) -> Vec<String> {
        all_tools.iter().filter(|name| self.is_allowed(name)).cloned().collect()
    }
    fn is_allowed(&self, tool_name: &str) -> bool {
        match self.mode {
            FilterMode::Whitelist => self.allowed.contains(tool_name),
            FilterMode::Blacklist => !self.denied.contains(tool_name),
        }
    }
}