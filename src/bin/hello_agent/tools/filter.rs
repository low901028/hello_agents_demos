use std::collections::HashSet;

/// 工具过滤器 trait
pub trait ToolFilter: Send + Sync {
    fn filter(&self, all_tools: &[String]) -> Vec<String>;
    fn is_allowed(&self, tool_name: &str) -> bool;
}

/// 只读过滤器
pub struct ReadOnlyFilter {
    allowed: HashSet<String>,
}

impl ReadOnlyFilter {
    pub fn new(additional: Option<Vec<String>>) -> Self {
        let mut allowed: HashSet<String> = [
            "Read", "ReadTool", "LS", "LSTool", "Glob", "GlobTool",
            "Grep", "GrepTool", "Skill", "SkillTool",
        ]
            .iter()
            .map(|s| s.to_string())
            .collect();

        if let Some(extra) = additional {
            allowed.extend(extra);
        }

        Self { allowed }
    }
}

impl ToolFilter for ReadOnlyFilter {
    fn filter(&self, all_tools: &[String]) -> Vec<String> {
        all_tools
            .iter()
            .filter(|t| self.is_allowed(t))
            .cloned()
            .collect()
    }

    fn is_allowed(&self, tool_name: &str) -> bool {
        self.allowed.contains(tool_name)
    }
}

/// 完全访问过滤器（黑名单模式）
pub struct FullAccessFilter {
    denied: HashSet<String>,
}

impl FullAccessFilter {
    pub fn new(additional_denied: Option<Vec<String>>) -> Self {
        let mut denied: HashSet<String> = [
            "Bash", "BashTool", "Terminal", "TerminalTool", "Execute", "ExecuteTool",
        ]
            .iter()
            .map(|s| s.to_string())
            .collect();

        if let Some(extra) = additional_denied {
            denied.extend(extra);
        }

        Self { denied }
    }
}

impl ToolFilter for FullAccessFilter {
    fn filter(&self, all_tools: &[String]) -> Vec<String> {
        all_tools
            .iter()
            .filter(|t| self.is_allowed(t))
            .cloned()
            .collect()
    }

    fn is_allowed(&self, tool_name: &str) -> bool {
        !self.denied.contains(tool_name)
    }
}

/// 自定义过滤器
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
    pub fn new(allowed: Option<Vec<String>>, denied: Option<Vec<String>>, mode: FilterMode) -> Self {
        Self {
            allowed: allowed.unwrap_or_default().into_iter().collect(),
            denied: denied.unwrap_or_default().into_iter().collect(),
            mode,
        }
    }
}

impl ToolFilter for CustomFilter {
    fn filter(&self, all_tools: &[String]) -> Vec<String> {
        all_tools
            .iter()
            .filter(|t| self.is_allowed(t))
            .cloned()
            .collect()
    }

    fn is_allowed(&self, tool_name: &str) -> bool {
        match self.mode {
            FilterMode::Whitelist => self.allowed.contains(tool_name),
            FilterMode::Blacklist => !self.denied.contains(tool_name),
        }
    }
}