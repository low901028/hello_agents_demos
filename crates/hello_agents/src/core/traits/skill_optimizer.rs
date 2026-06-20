use async_trait::async_trait;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::skill_opt::{Trajectory, EditProposal, Skill, Task};

/// 技能优化器接口（对应 SkillOpt 论文的核心优化循环）
#[async_trait]
pub trait SkillOptimizer: Send + Sync {
    /// Rollout：执行一批任务，返回带分数的轨迹
    async fn rollout(&self, skill: &Skill, tasks: &[Task]) -> Result<Vec<Trajectory>, HelloAgentError>;

    /// Reflect：分析轨迹生成编辑提案（双轨反思）
    async fn reflect(&self, trajectories: &[Trajectory]) -> Result<Vec<EditProposal>, HelloAgentError>;

    /// Aggregate：合并、去重、归纳泛化
    async fn aggregate(&self, proposals: Vec<EditProposal>) -> Result<Vec<EditProposal>, HelloAgentError>;

    /// Clip：按编辑预算排序截断
    fn clip(&self, proposals: Vec<EditProposal>, budget: usize) -> Vec<EditProposal>;

    /// Update：应用编辑到技能文档
    async fn update(&self, skill: &mut Skill, edits: &[EditProposal]) -> Result<(), HelloAgentError>;

    /// Gate：验证集门控，返回是否接受
    async fn gate(&self, candidate: &Skill, current: &Skill, val_tasks: &[Task]) -> Result<bool, HelloAgentError>;

    /// 执行一步完整的进化（Rollout → Reflect → Aggregate → Clip → Update → Gate）
    async fn evolve_step(
        &self,
        skill: &mut Skill,
        train_tasks: &[Task],
        val_tasks: &[Task],
        budget: usize,
    ) -> Result<bool, HelloAgentError>;

    /// Epoch 级慢更新
    async fn slow_update(&self, prev_skill: &Skill, current_skill: &Skill, tasks: &[Task]) -> Result<(), HelloAgentError>;

    /// 更新元技能
    async fn update_meta_skill(&self, edits: &[EditProposal], accepted: bool) -> Result<(), HelloAgentError>;

    /// 记录单次技能执行结果（用于后续优化，非论文核心循环，供 SkillTool 调用）
    async fn record_execution(&self, skill_name: &str, result: &SkillExecutionResult) -> Result<(), HelloAgentError>;

    /// 获取技能统计
    fn get_stats(&self, skill_name: &str) -> Option<SkillStats>;
}

/// 技能执行结果（供 SkillTool 反馈用）
#[derive(Debug, Clone)]
pub struct SkillExecutionResult {
    pub success: bool,
    pub score: Option<f64>,
    pub latency: std::time::Duration,
    pub tokens_used: usize,
    pub error_message: Option<String>,
}

/// 技能统计
#[derive(Debug, Clone)]
pub struct SkillStats {
    pub call_count: u64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub avg_tokens: f64,
}