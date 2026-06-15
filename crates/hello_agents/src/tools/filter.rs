//! ============================================================
//! src/tools/filter.rs
//! ============================================================
use crate::core::traits::tool_filter::ToolFilter;
use std::collections::HashSet;

const READONLY_TOOLS: &[&str] = &[
    "Read",
    "ReadTool",
    "LS",
    "LSTool",
    "Glob",
    "GlobTool",
    "Grep",
    "GrepTool",
    "Skill",
    "SkillTool",
];

pub struct ReadOnlyFilter {
    allowed: HashSet<String>,
}

impl ReadOnlyFilter {
    pub fn new(additional: Option<Vec<String>>) -> Self {
        let mut allowed: HashSet<String> = READONLY_TOOLS.iter().map(|s| s.to_string()).collect();
        if let Some(extra) = additional {
            allowed.extend(extra);
        }
        Self { allowed }
    }
}

impl ToolFilter for ReadOnlyFilter {
    fn filter(&self, all: &[String]) -> Vec<String> {
        all.iter().filter(|n| self.is_allowed(n)).cloned().collect()
    }
    fn is_allowed(&self, name: &str) -> bool {
        self.allowed.contains(name)
    }
}

const DENIED_TOOLS: &[&str] = &[
    "Bash",
    "BashTool",
    "Terminal",
    "TerminalTool",
    "Execute",
    "ExecuteTool",
];

pub struct FullAccessFilter {
    denied: HashSet<String>,
}

impl FullAccessFilter {
    pub fn new(additional: Option<Vec<String>>) -> Self {
        let mut denied: HashSet<String> = DENIED_TOOLS.iter().map(|s| s.to_string()).collect();
        if let Some(extra) = additional {
            denied.extend(extra);
        }
        Self { denied }
    }
}

impl ToolFilter for FullAccessFilter {
    fn filter(&self, all: &[String]) -> Vec<String> {
        all.iter().filter(|n| self.is_allowed(n)).cloned().collect()
    }
    fn is_allowed(&self, name: &str) -> bool {
        !self.denied.contains(name)
    }
}

pub struct CustomFilter {
    allowed: HashSet<String>,
    denied: HashSet<String>,
    mode: FilterMode,
}

pub enum FilterMode {
    Whitelist,
    Blacklist,
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
    fn filter(&self, all: &[String]) -> Vec<String> {
        all.iter().filter(|n| self.is_allowed(n)).cloned().collect()
    }
    fn is_allowed(&self, name: &str) -> bool {
        match self.mode {
            FilterMode::Whitelist => self.allowed.contains(name),
            FilterMode::Blacklist => !self.denied.contains(name),
        }
    }
}
