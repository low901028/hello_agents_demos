pub struct SkillOptConfig {
    pub max_edit_budget: usize,         // 默认4
    pub budget_schedule: BudgetSchedule,
    pub minibatch_size: usize,          // 默认8
    pub max_epochs: usize,
    pub val_ratio: f64,                 // 验证集比例
    pub enable_slow_update: bool,
    pub enable_meta_skill: bool,
    pub early_stop_patience: usize,
}

pub enum BudgetSchedule {
    Constant,
    LinearDecay,
    Cosine,
}

impl Default for SkillOptConfig {
    fn default() -> Self {
        Self {
            max_edit_budget: 4,
            budget_schedule: BudgetSchedule::Cosine,
            minibatch_size: 8,
            max_epochs: 10,
            val_ratio: 0.2,
            enable_slow_update: true,
            enable_meta_skill: true,
            early_stop_patience: 3,
        }
    }
}