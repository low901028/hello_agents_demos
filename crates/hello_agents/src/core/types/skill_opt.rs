use serde::{Deserialize, Serialize};
use std::time::Duration;

/// 一条执行轨迹
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub task_id: String,
    pub messages: Vec<String>,
    pub tool_calls: Vec<String>,
    pub final_answer: String,
    pub score: f64,
    pub success: bool,
    pub error_message: Option<String>,
    pub latency: Duration,
    pub tokens_used: usize,
}

/// 编辑操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditOperation {
    Add,
    Delete,
    Replace,
    InsertAfter,
}

/// 编辑来源
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EditSource {
    Failure,
    Success,
}

/// 单个编辑提案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditProposal {
    pub op: EditOperation,
    pub content: String,
    pub target: Option<String>,
    pub support_count: usize,
    pub source_type: EditSource,
    pub expected_impact: String,
}

/// 元技能状态（仅优化器侧，不部署）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaSkill {
    pub effective_patterns: Vec<String>,
    pub rejected_patterns: Vec<String>,
    pub persistent_failures: Vec<String>,
    pub epoch: usize,
}

impl MetaSkill {
    pub fn new() -> Self {
        Self {
            effective_patterns: vec![],
            rejected_patterns: vec![],
            persistent_failures: vec![],
            epoch: 0,
        }
    }

    pub fn record_success(&mut self, edits: &[EditProposal]) {
        for e in edits {
            self.effective_patterns.push(e.content.clone());
        }
    }

    pub fn record_rejection(&mut self, edits: &[EditProposal]) {
        for e in edits {
            self.rejected_patterns.push(e.content.clone());
        }
    }
}

/// 技能文档（对接 SkillLoader 的 Skill）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,                // 包含 <!-- SLOW_UPDATE_START/END --> 标记
    pub dir: String,
}

/// 任务定义
#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub input: String,
    pub expected_output: Option<String>,
    pub metadata: serde_json::Value,
}