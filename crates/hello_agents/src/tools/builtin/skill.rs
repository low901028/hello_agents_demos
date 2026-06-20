use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use serde_json::Value;

use crate::core::traits::tool::Tool;
use crate::core::traits::skill_optimizer::{SkillExecutionResult, SkillOptimizer};
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::{ToolResponse, ToolStatus};
use crate::skills::skill_loader::SkillLoader;

pub struct SkillTool {
    loader: Arc<Mutex<SkillLoader>>,
    /// 可选：技能优化器，用于记录执行反馈
    skill_optimizer: Option<Arc<dyn SkillOptimizer>>,
}

impl SkillTool {
    pub fn new(loader: Arc<Mutex<SkillLoader>>) -> Self {
        Self {
            loader,
            skill_optimizer: None,
        }
    }

    /// 注入技能优化器
    pub fn with_skill_optimizer(mut self, optimizer: Arc<dyn SkillOptimizer>) -> Self {
        self.skill_optimizer = Some(optimizer);
        self
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str { "Skill" }
    fn description(&self) -> &str { "加载技能获取专业知识" }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill": { "type": "string", "description": "要加载的技能名称" },
                "args": { "type": "string", "description": "可选参数，替换 $ARGUMENTS 占位符", "default": "" }
            },
            "required": ["skill"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let skill_name = args["skill"].as_str().unwrap_or("");
        let args_str = args.get("args").and_then(|v| v.as_str()).unwrap_or("");
        let start = Instant::now();

        if skill_name.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "技能名称不能为空"));
        }

        // 使用块作用域确保锁在异步调用前释放
        let response = {
            let mut loader = self.loader.lock().unwrap();
            match loader.get_skill(skill_name) {
                Some(skill) => {
                    let content = skill.body.replace("$ARGUMENTS", args_str);
                    let full = format!(
                        "<skill-loaded name=\"{}\">\n{}\n</skill-loaded>",
                        skill_name, content
                    );
                    ToolResponse::success(full)
                }
                None => ToolResponse::error(
                    "NOT_FOUND",
                    &format!("技能 '{}' 不存在", skill_name),
                ),
            }
        }; // 锁在此处自动释放

        // 异步记录反馈
        if let Some(optimizer) = &self.skill_optimizer {
            let result = SkillExecutionResult {
                success: response.status == ToolStatus::Success,
                score: None,
                latency: start.elapsed(),
                tokens_used: response.text.len(),
                error_message: response.error_info.clone().map(|e| e.message),
            };
            let _ = optimizer.record_execution(skill_name, &result).await;
        }

        Ok(response)
    }
}