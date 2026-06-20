use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;

use crate::core::traits::skill_optimizer::{SkillOptimizer, SkillExecutionResult, SkillStats};
use crate::core::traits::llm_provider::LlmProvider;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::message::Message;
use crate::core::types::skill_opt::{
    Trajectory, EditProposal, EditOperation, EditSource, Skill, Task, MetaSkill,
};
use crate::infra::skill_opt_config::SkillOptConfig;
use crate::skills::skill_loader::SkillLoader;

// 中文 Prompt 常量（完整版，对应论文 Appendix C）
const FAILURE_ANALYST_SYSTEM: &str = r#"
你是一位智能体任务失败分析专家。

你将获得**同一小批次中的多条失败轨迹**以及当前的技能文档。你的任务是找出这批轨迹中**最重要的共性失败模式**，并提出一套简洁的技能编辑方案。

## 分析流程
1. 阅读小批次中的所有轨迹。
2. 识别其中最常见、最系统的失败模式。
3. 对每种模式进行失败类型分类。
4. 提出针对**共性模式**的技能编辑，不要针对个别边缘案例。
5. 编辑必须具有泛化性，禁止硬编码具体任务值。
6. 只修补技能中的缺失，不要重复已有内容。

你会被告知最大编辑数量（预算 L）。请生成**至多 L 条**编辑，聚焦于最普遍适用的模式。

**重要**：技能文档中可能包含由 `<!-- SLOW_UPDATE_START -->` 和 `<!-- SLOW_UPDATE_END -->` 标记的保护区域。该区域由独立的慢更新过程管理，**请勿**建议任何针对该区域的编辑。

请**只输出**一个合法的 JSON 对象，格式如下：
{
  "patch": {
    "reasoning": "<解释为何这些编辑能解决共性问题>",
    "edits": [
      {"op": "append", "content": "<要追加的 Markdown 内容>"},
      {"op": "insert_after", "target": "<精确的标题或文本>", "content": "<要插入的内容>"},
      {"op": "replace", "target": "<要被替换的精确文本>", "content": "<替换后的内容>"},
      {"op": "delete", "target": "<要删除的精确文本>"}
    ]
  }
}
"#;

const SUCCESS_ANALYST_SYSTEM: &str = r#"
你是一位智能体成功模式分析专家。

你将获得**同一小批次中的多条成功轨迹**以及当前的技能文档。你的任务是识别这批轨迹中的**通用行为模式**，如果技能尚未覆盖这些模式，就将其编码进技能中。

## 规则
- 只针对技能中**尚未覆盖**的模式提出修补。
- 聚焦于**多条轨迹中共现**的模式。
- 内容要简洁，模式必须能泛化到具体任务之外。
- 优先在已有段落中强化，而非新增顶层章节。

你会被告知最大编辑数量（预算 L）。请生成**至多 L 条**编辑，聚焦于最通用的模式。

**重要**：同样禁止修改 `<!-- SLOW_UPDATE_START -->` 和 `<!-- SLOW_UPDATE_END -->` 之间的保护区。

请只输出合法的 JSON 对象：
{
  "patch": {
    "reasoning": "<为何这些模式值得编码>",
    "edits": [ ... ]  // 同上
  }
}
"#;

const MERGE_FAILURE_SYSTEM: &str = r#"
你是一位技能编辑协调员。你收到多份来自**失败分析**的独立编辑提案，请将它们合并成一个连贯的补丁。

合并原则：
1. 去重：保留表述最清晰、最泛化的版本。
2. 解决冲突：如果提案在同一问题上矛盾，选择论证更充分的，或综合二者。
3. 保留独到见解：包含所有非冗余的纠正性编辑。
4. 共性优先：出现在多个提案中的编辑代表系统性问题，应**高优先级**保留；仅出现在单个提案中的编辑可能是个例，可舍弃。
5. 独立性：合并后的补丁中，任何两条编辑不能作用于同一文本区域。
6. 支持计数：对每条合并后的编辑，估算有多少源提案支持它。
7. **保护区**：禁止编辑 `<!-- SLOW_UPDATE_START -->` 和 `<!-- SLOW_UPDATE_END -->` 之间的内容。

输出格式：
{
  "reasoning": "<合并决策摘要>",
  "edits": [
    {
      "op": "append|insert_after|replace|delete",
      "target": "<必要时填写>",
      "content": "<内容>",
      "support_count": <整数>,
      "source_type": "failure"
    }
  ]
}
"#;

const MERGE_SUCCESS_SYSTEM: &str = r#"
你是一位技能编辑协调员。你收到多份来自**成功分析**的独立编辑提案，请合并它们以强化有效模式。

合并原则：
1. 去重：仅保留最通用的模式版本。
2. 保守：成功驱动的修补是为了强化已有行为，只包含技能中**尚未覆盖**的模式。
3. 共性优先：在多个成功轨迹中出现的模式最值得编码。
4. 支持计数：估算每条合并编辑的支持提案数。
5. **保护区**：禁止编辑保护区。

输出格式同 MERGE_FAILURE_SYSTEM，但 `source_type` 为 "success"。
"#;

const FINAL_MERGE_SYSTEM: &str = r#"
你是一位技能编辑协调员。你现在有两组合并后的编辑：一组来自失败分析（已合并），另一组来自成功分析（已合并）。请将它们合并为最终的补丁。

优先级规则：
1. **失败修补优先**：技能反思的首要目标是修复失败。因此，失败驱动的编辑应予以保留，除非它们与一个论证充分的成功模式直接冲突。
2. 去重：如果失败编辑和成功编辑覆盖同一要点，保留失败版本。
3. 保留成功洞见：包含那些未被失败编辑覆盖的成功模式。
4. 高层合并代表更广泛的共识：在先前合并轮次中幸存下来的编辑应获得更高优先级。
5. 携带 `support_count` 和 `source_type`。
6. **保护区**：禁止编辑保护区。

输出格式同上。
"#;

const RANKING_SYSTEM: &str = r#"
你是一位技能编辑排序优化专家。你将获得当前技能文档和一组待选编辑。请按重要性对编辑排序，并选择最靠前的若干条。

排序标准（按优先级降序）：
1. **系统性影响**：解决跨任务广泛、反复出现的失败模式的编辑排名最高。能修复 50% 失败的规则优于只修复单个边界情况的规则。
2. **互补性**：填补当前技能空缺、而非重复已有内容的编辑排名更高。
3. **泛化性**：表述为通用原则的编辑排名高于针对特定问题类型或实体的编辑。
4. **可操作性**：提供清晰、具体指导的编辑排名高于模糊建议。

你会被告知需要选择的数量（预算）。

输出 JSON：
{
  "reasoning": "<排序决策的简要理由>",
  "selected_indices": [<选中的编辑在输入数组中的 0-based 索引，按优先级从高到低>]
}
"#;

pub struct LLMDrivenSkillOptimizer {
    target_llm: Arc<dyn LlmProvider>,
    optimizer_llm: Arc<dyn LlmProvider>,
    skill_loader: Arc<Mutex<SkillLoader>>,
    rejected_buffer: Mutex<Vec<EditProposal>>,
    meta_skill: Mutex<MetaSkill>,
    execution_logs: Mutex<HashMap<String, Vec<SkillExecutionResult>>>,
    config: SkillOptConfig,
}

impl LLMDrivenSkillOptimizer {
    pub fn new(
        target_llm: Arc<dyn LlmProvider>,
        optimizer_llm: Arc<dyn LlmProvider>,
        skill_loader: Arc<Mutex<SkillLoader>>,
        config: SkillOptConfig,
    ) -> Self {
        Self {
            target_llm,
            optimizer_llm,
            skill_loader,
            rejected_buffer: Mutex::new(Vec::new()),
            meta_skill: Mutex::new(MetaSkill::new()),
            execution_logs: Mutex::new(HashMap::new()),
            config,
        }
    }

    /// 余弦调度编辑预算
    pub fn cosine_budget(step: usize, max_steps: usize, max_budget: usize) -> usize {
        let p = (step as f64 / max_steps as f64 * std::f64::consts::PI / 2.0).cos();
        (max_budget as f64 * p).round().max(1.0) as usize
    }

    /// 执行单个任务并返回轨迹（简化模拟，实际需接入执行环境）
    async fn execute_task(&self, skill: &Skill, task: &Task) -> Result<Trajectory, HelloAgentError> {
        let messages = vec![
            Message::system(&skill.body),
            Message::user(&task.input),
        ];
        let resp = self.target_llm.chat(&messages, None, None).await?;
        let final_answer = resp.content.unwrap_or_default();
        let score = if let Some(expected) = &task.expected_output {
            if final_answer.contains(expected) { 1.0 } else { 0.0 }
        } else { 0.5 };
        Ok(Trajectory {
            task_id: task.id.clone(),
            messages: vec![task.input.clone()],
            tool_calls: vec![],
            final_answer,
            score,
            success: score > 0.5,
            error_message: None,
            latency: std::time::Duration::from_millis(100),
            tokens_used: 50,
        })
    }

    /// 调用优化器模型分析失败 minibatch（中文 Prompt）
    async fn analyze_failures(
        &self,
        batch: &[Trajectory],
        rejected: &[EditProposal],
    ) -> Result<Vec<EditProposal>, HelloAgentError> {
        let context = format!(
            "你是一名错误分析师。以下是失败的轨迹。请识别系统性失败模式并提出编辑。\n最近被拒绝的编辑（不要重复）：{:?}\n\n轨迹：\n{}",
            rejected.iter().map(|e| e.content.clone()).collect::<Vec<_>>(),
            batch.iter().map(|t| t.final_answer.clone()).collect::<Vec<_>>().join("\n")
        );
        let resp = self.optimizer_llm.chat(
            &[Message::system(FAILURE_ANALYST_SYSTEM), Message::user(&context)],
            None, None,
        ).await?;
        Ok(vec![EditProposal {
            op: EditOperation::Add,
            content: resp.content.unwrap_or_default(),
            target: None,
            support_count: batch.len(),
            source_type: EditSource::Failure,
            expected_impact: "修复系统性错误".into(),
        }])
    }

    /// 调用优化器模型分析成功 minibatch（中文 Prompt）
    async fn analyze_successes(&self, batch: &[Trajectory]) -> Result<Vec<EditProposal>, HelloAgentError> {
        let context = format!(
            "你是一名成功分析师。以下是成功的轨迹。请识别有效的模式。\n\n轨迹：\n{}",
            batch.iter().map(|t| t.final_answer.clone()).collect::<Vec<_>>().join("\n")
        );
        let resp = self.optimizer_llm.chat(
            &[Message::system(SUCCESS_ANALYST_SYSTEM), Message::user(&context)],
            None, None,
        ).await?;
        Ok(vec![EditProposal {
            op: EditOperation::Add,
            content: resp.content.unwrap_or_default(),
            target: None,
            support_count: batch.len(),
            source_type: EditSource::Success,
            expected_impact: "保留有效模式".into(),
        }])
    }

    /// 计算技能在一组任务上的平均分数
    async fn evaluate(&self, skill: &Skill, tasks: &[Task]) -> Result<f64, HelloAgentError> {
        let mut total = 0.0;
        for task in tasks {
            let traj = self.execute_task(skill, task).await?;
            total += traj.score;
        }
        Ok(total / tasks.len() as f64)
    }
}

#[async_trait]
impl SkillOptimizer for LLMDrivenSkillOptimizer {
    async fn rollout(&self, skill: &Skill, tasks: &[Task]) -> Result<Vec<Trajectory>, HelloAgentError> {
        let mut trajectories = Vec::new();
        for task in tasks {
            trajectories.push(self.execute_task(skill, task).await?);
        }
        Ok(trajectories)
    }

    async fn reflect(&self, trajectories: &[Trajectory]) -> Result<Vec<EditProposal>, HelloAgentError> {
        let (failures, successes): (Vec<_>, Vec<_>) =
            trajectories.iter().cloned().partition(|t| !t.success);
        let mut proposals = Vec::new();
        let rejected = self.rejected_buffer.lock().unwrap().clone();

        for batch in failures.chunks(self.config.minibatch_size) {
            let edits = self.analyze_failures(batch, &rejected).await?;
            proposals.extend(edits);
        }
        for batch in successes.chunks(self.config.minibatch_size) {
            let edits = self.analyze_successes(batch).await?;
            proposals.extend(edits);
        }
        Ok(proposals)
    }

    async fn aggregate(&self, mut proposals: Vec<EditProposal>) -> Result<Vec<EditProposal>, HelloAgentError> {
        proposals.sort_by(|a, b| b.support_count.cmp(&a.support_count));
        proposals.dedup_by(|a, b| a.content == b.content);
        let mut fails: Vec<_> = proposals.iter().filter(|p| p.source_type == EditSource::Failure).cloned().collect();
        let succs: Vec<_> = proposals.iter().filter(|p| p.source_type == EditSource::Success).cloned().collect();
        fails.extend(succs);
        Ok(fails)
    }

    fn clip(&self, proposals: Vec<EditProposal>, budget: usize) -> Vec<EditProposal> {
        let mut sorted = proposals;
        sorted.sort_by(|a, b| b.support_count.cmp(&a.support_count));
        sorted.truncate(budget);
        sorted
    }

    async fn update(&self, skill: &mut Skill, edits: &[EditProposal]) -> Result<(), HelloAgentError> {
        for edit in edits {
            match edit.op {
                EditOperation::Add | EditOperation::InsertAfter => {
                    skill.body.push_str(&format!("\n{}", edit.content));
                }
                EditOperation::Delete => {
                    if let Some(target) = &edit.target {
                        skill.body = skill.body.replace(target.as_str(), "");
                    }
                }
                EditOperation::Replace => {
                    if let Some(target) = &edit.target {
                        skill.body = skill.body.replace(target.as_str(), &edit.content);
                    }
                }
            }
        }
        Ok(())
    }

    async fn gate(&self, candidate: &Skill, current: &Skill, val_tasks: &[Task]) -> Result<bool, HelloAgentError> {
        let cand_score = self.evaluate(candidate, val_tasks).await?;
        let curr_score = self.evaluate(current, val_tasks).await?;
        Ok(cand_score > curr_score)
    }

    async fn evolve_step(
        &self,
        skill: &mut Skill,
        train_tasks: &[Task],
        val_tasks: &[Task],
        budget: usize,
    ) -> Result<bool, HelloAgentError> {
        println!("====evolve_step enter...");
        let trajectories = self.rollout(skill, train_tasks).await?;
        println!("====rollout completed...");
        let raw_edits = self.reflect(&trajectories).await?;
        println!("====reflect completed...");
        let aggregated = self.aggregate(raw_edits).await?;
        println!("====aggregate completed...");
        let selected = self.clip(aggregated, budget);
        println!("====clip completed...");
        let mut candidate = skill.clone();
        self.update(&mut candidate, &selected).await?;
        println!("====update completed...");
        if self.gate(&candidate, skill, val_tasks).await? {
            *skill = candidate;
            self.update_meta_skill(&selected, true).await?;
            Ok(true)
        } else {
            self.rejected_buffer.lock().unwrap().extend(selected.clone());
            self.update_meta_skill(&selected, false).await?;
            Ok(false)
        }
    }

    async fn slow_update(&self, prev_skill: &Skill, current_skill: &Skill, tasks: &[Task]) -> Result<(), HelloAgentError> {
        let prev_score = self.evaluate(prev_skill, tasks).await?;
        let curr_score = self.evaluate(current_skill, tasks).await?;
        println!("慢更新：上一轮分数={:.2}，当前分数={:.2}", prev_score, curr_score);
        Ok(())
    }

    async fn update_meta_skill(&self, edits: &[EditProposal], accepted: bool) -> Result<(), HelloAgentError> {
        let mut meta = self.meta_skill.lock().unwrap();
        if accepted {
            meta.record_success(edits);
        } else {
            meta.record_rejection(edits);
        }
        Ok(())
    }

    async fn record_execution(&self, skill_name: &str, result: &SkillExecutionResult) -> Result<(), HelloAgentError> {
        let mut logs = self.execution_logs.lock().unwrap();
        logs.entry(skill_name.to_string())
            .or_default()
            .push(result.clone());
        Ok(())
    }

    fn get_stats(&self, skill_name: &str) -> Option<SkillStats> {
        let logs = self.execution_logs.lock().unwrap();
        let entries = logs.get(skill_name)?;
        let count = entries.len() as u64;
        let success_rate = entries.iter().filter(|e| e.success).count() as f64 / count as f64;
        let avg_latency = entries.iter().map(|e| e.latency.as_millis() as f64).sum::<f64>() / count as f64;
        let avg_tokens = entries.iter().map(|e| e.tokens_used as f64).sum::<f64>() / count as f64;
        Some(SkillStats {
            call_count: count,
            success_rate,
            avg_latency_ms: avg_latency,
            avg_tokens,
        })
    }
}