// ============================================================
// agents/plan_solve.rs
// Plan‑and‑Solve Agent – 分解规划与逐步执行
// ============================================================

use crate::core::agent_runtime::AgentRuntime;
use crate::core::traits::agent::Agent;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::message::Message;
use async_trait::async_trait;

pub struct PlanSolveAgent {
    name: String,
    system_prompt: Option<String>,
}

impl PlanSolveAgent {
    pub fn new(name: &str, system_prompt: Option<String>) -> Self {
        Self {
            name: name.into(),
            system_prompt,
        }
    }
}

#[async_trait]
impl Agent for PlanSolveAgent {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run(
        &mut self,
        input: &str,
        runtime: &AgentRuntime,
    ) -> Result<String, HelloAgentError> {
        let sys = self
            .system_prompt
            .clone()
            .unwrap_or_else(|| "你是一个规划-执行助手".into());

        // 1. 制定计划
        let plan_prompt = format!("{sys}\n\n请为以下任务制定计划：{input}");
        let plan_resp = runtime
            .llm
            .chat(&[Message::user(&plan_prompt)], None, None)
            .await?;
        let plan = plan_resp.content.unwrap_or_default();

        // 2. 执行计划
        let exec_prompt = format!("{sys}\n\n计划：{plan}\n\n请按计划执行并给出最终答案：");
        let exec_resp = runtime
            .llm
            .chat(&[Message::user(&exec_prompt)], None, None)
            .await?;
        Ok(exec_resp.content.unwrap_or_default())
    }
}
