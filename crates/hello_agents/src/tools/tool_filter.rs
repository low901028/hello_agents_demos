//! tool_filter.rs
//! 工具响应协议
//!
//! 标准化的工具响应格式，提供结构化的状态、数据和错误信息。
//!
use std::collections::HashSet;

/// 工具过滤器 trait
/// 用于在子代理运行时限制可用工具集合
pub trait ToolFilter: Send + Sync {
    /// 过滤工具列表
    ///
    ///         Args:
    ///             all_tools: 所有可用工具名称列表
    ///
    ///         Returns:
    ///             过滤后的工具名称列表
    fn filter(&self, all_tools: &[String]) -> Vec<String>;

    /// 检查单个工具是否允许
    ///
    ///         Args:
    ///             tool_name: 工具名称
    ///
    ///         Returns:
    ///             是否允许使用该工具
    fn is_allowed(&self, tool_name: &str) -> bool;

    /// 通用过滤
    fn base_filter(&self, all_tools: &[String]) -> Vec<String> {
        all_tools
            .iter()
            .filter(|name| self.is_allowed(name))
            .cloned()
            .collect()
    }
}

// ---------------------------------------------------------------
// 只读工具过滤器
// ---------------------------------------------------------------
/// 只读工具白名单
///
///     只允许使用只读工具，适用于：
///     - explore（探索代码库）
///     - plan（规划任务）
///     - summary（归纳信息）
const READONLY_TOOLS: &[&str] = &[
    "Read", "ReadTool",
    "LS", "LSTool",
    "Glob", "GlobTool",
    "Grep", "GrepTool",
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
        self.base_filter(all_tools)
    }

    fn is_allowed(&self, tool_name: &str) -> bool {
        self.allowed.contains(tool_name)
    }
}

// ---------------------------------------------------------------
// 完全访问过滤器（排除危险工具）
// ---------------------------------------------------------------

//危险工具黑名单
const DENIED_TOOLS: &[&str] = &[
    "Bash", "BashTool",
    "Terminal", "TerminalTool",
    "Execute", "ExecuteTool",
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
        self.base_filter(all_tools)
    }

    fn is_allowed(&self, tool_name: &str) -> bool {
        !self.denied.contains(tool_name)
    }
}

// ---------------------------------------------------------------
// 自定义过滤器
// ---------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Whitelist,
    Blacklist,
}

/// 自定义工具过滤器
///
///     用户可以明确指定允许或禁止的工具列表
///     allowed: 允许的工具名称列表（白名单模式）
///     denied: 禁止的工具名称列表（黑名单模式）
///     mode: 过滤模式，"whitelist"（白名单）或 "blacklist"（黑名单）
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
        all_tools
            .iter()
            .filter(|name| self.is_allowed(name))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<String> {
        vec![
            "Read".to_string(),
            "ReadTool".to_string(),
            "LS".to_string(),
            "Glob".to_string(),
            "Bash".to_string(),
            "BashTool".to_string(),
            "Execute".to_string(),
            "CustomTool".to_string(),
        ]
    }

    // ---------- ReadOnlyFilter ----------
    #[test]
    fn readonly_filter_permits_only_readonly() {
        let f = ReadOnlyFilter::new(None);
        let result = f.filter(&tools());
        let expected: Vec<&str> = vec!["Read", "ReadTool", "LS", "Glob"];
        assert_eq!(result, expected);
    }

    #[test]
    fn readonly_filter_with_additional() {
        let f = ReadOnlyFilter::new(Some(vec!["CustomTool".into()]));
        let result = f.filter(&tools());
        let expected: Vec<&str> = vec!["Read", "ReadTool", "LS", "Glob", "CustomTool"];
        assert_eq!(result, expected);
    }

    #[test]
    fn readonly_filter_is_allowed() {
        let f = ReadOnlyFilter::new(None);
        assert!(f.is_allowed("Read"));
        assert!(f.is_allowed("LS"));
        assert!(!f.is_allowed("Bash"));
        assert!(!f.is_allowed("CustomTool"));
    }

    // ---------- FullAccessFilter ----------
    #[test]
    fn fullaccess_filter_denies_dangerous() {
        let f = FullAccessFilter::new(None);
        let result = f.filter(&tools());
        let expected: Vec<&str> = vec!["Read", "ReadTool", "LS", "Glob", "CustomTool"];
        assert_eq!(result, expected);
    }

    #[test]
    fn fullaccess_filter_with_additional_denied() {
        let f = FullAccessFilter::new(Some(vec!["CustomTool".into()]));
        let result = f.filter(&tools());
        let expected: Vec<&str> = vec!["Read", "ReadTool", "LS", "Glob"];
        assert_eq!(result, expected);
    }

    #[test]
    fn fullaccess_filter_is_allowed() {
        let f = FullAccessFilter::new(None);
        assert!(f.is_allowed("Read"));
        assert!(!f.is_allowed("Bash"));
        assert!(!f.is_allowed("Execute"));
        assert!(f.is_allowed("CustomTool"));
    }

    // ---------- CustomFilter ----------
    #[test]
    fn custom_filter_whitelist() {
        let f = CustomFilter::new(
            Some(vec!["Read".into(), "LS".into()]),
            None,
            FilterMode::Whitelist,
        );
        let result = f.filter(&tools());
        assert_eq!(result, vec!["Read", "LS"]);
    }

    #[test]
    fn custom_filter_blacklist() {
        let f = CustomFilter::new(
            None,
            Some(vec!["Bash".into(), "BashTool".into()]),
            FilterMode::Blacklist,
        );
        let result = f.filter(&tools());
        let expected: Vec<&str> = vec!["Read", "ReadTool", "LS", "Glob", "Execute", "CustomTool"];
        assert_eq!(result, expected);
    }

    #[test]
    fn custom_filter_whitelist_is_allowed() {
        let f = CustomFilter::new(
            Some(vec!["Read".into()]),
            None,
            FilterMode::Whitelist,
        );
        assert!(f.is_allowed("Read"));
        assert!(!f.is_allowed("LS"));
    }

    #[test]
    fn custom_filter_blacklist_is_allowed() {
        let f = CustomFilter::new(
            None,
            Some(vec!["Bash".into()]),
            FilterMode::Blacklist,
        );
        assert!(!f.is_allowed("Bash"));
        assert!(f.is_allowed("Read"));
    }

    #[test]
    fn custom_filter_invalid_mode_should_be_avoided() {
        // The Rust enum ensures valid mode at compile time, so no runtime error.
        // We just verify construction works.
        let f = CustomFilter::new(None, None, FilterMode::Whitelist);
        assert!(f.is_allowed("anything") == false); // empty whitelist -> nothing allowed
        let f = CustomFilter::new(None, None, FilterMode::Blacklist);
        assert!(f.is_allowed("anything")); // empty blacklist -> everything allowed
    }
}