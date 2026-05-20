use std::collections::HashSet;

pub trait ToolFilter: Send + Sync {
    fn filter(&self, all_tools: &[String]) -> Vec<String>;
    fn is_allowed(&self, tool_name: &str) -> bool;
}

#[derive(Debug, Clone)]
pub struct ReadOnlyFilter {
    allowed: HashSet<String>,
}

impl ReadOnlyFilter {
    pub fn new(additional: Option<Vec<String>>) -> Self {
        let mut allowed: HashSet<String> = [
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
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        if let Some(extra) = additional {
            allowed.extend(extra);
        }
        ReadOnlyFilter { allowed }
    }
}

impl ToolFilter for ReadOnlyFilter {
    fn filter(&self, all: &[String]) -> Vec<String> {
        all.iter().filter(|t| self.is_allowed(t)).cloned().collect()
    }
    fn is_allowed(&self, t: &str) -> bool {
        self.allowed.contains(t)
    }
}

impl Default for ReadOnlyFilter {
    fn default() -> Self {
        ReadOnlyFilter::new(None)
    }
}

#[derive(Debug, Clone)]
pub struct FullAccessFilter {
    denied: HashSet<String>,
}

impl FullAccessFilter {
    pub fn new(additional: Option<Vec<String>>) -> Self {
        let mut denied: HashSet<String> = [
            "Bash",
            "BashTool",
            "Terminal",
            "TerminalTool",
            "Execute",
            "ExecuteTool",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        if let Some(extra) = additional {
            denied.extend(extra);
        }
        FullAccessFilter { denied }
    }
}

impl ToolFilter for FullAccessFilter {
    fn filter(&self, all: &[String]) -> Vec<String> {
        all.iter().filter(|t| self.is_allowed(t)).cloned().collect()
    }
    fn is_allowed(&self, t: &str) -> bool {
        !self.denied.contains(t)
    }
}

impl Default for FullAccessFilter {
    fn default() -> Self {
        FullAccessFilter::new(None)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilterMode {
    Whitelist,
    Blacklist,
}

#[derive(Debug, Clone)]
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
        CustomFilter {
            allowed: allowed.unwrap_or_default().into_iter().collect(),
            denied: denied.unwrap_or_default().into_iter().collect(),
            mode,
        }
    }
    pub fn whitelist(allowed: Vec<String>) -> Self {
        Self::new(Some(allowed), None, FilterMode::Whitelist)
    }
    pub fn blacklist(denied: Vec<String>) -> Self {
        Self::new(None, Some(denied), FilterMode::Blacklist)
    }
}

impl ToolFilter for CustomFilter {
    fn filter(&self, all: &[String]) -> Vec<String> {
        all.iter().filter(|t| self.is_allowed(t)).cloned().collect()
    }
    fn is_allowed(&self, t: &str) -> bool {
        match self.mode {
            FilterMode::Whitelist => self.allowed.contains(t),
            FilterMode::Blacklist => !self.denied.contains(t),
        }
    }
}
