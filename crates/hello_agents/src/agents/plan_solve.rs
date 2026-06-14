use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;

use crate::agents::base::AgentBase;
use crate::core::traits::agent::{Agent, SubagentMetadata, SubagentResult};
use crate::core::traits::context::AgentContext;
use crate::core::types::config::Config;
use crate::core::types::exceptions::HelloAgentException;
use crate::core::types::message::{Message, MessageContent, MessageRole};
use crate::infra::llm::hello_agents_llm::HelloAgentsLLM;
use crate::tools::filter::ToolFilter;
use crate::tools::registry::ToolRegistry;
use crate::tools::response::ToolStatus;

// -------------------- Planner --------------------
pub struct Planner {
    llm: HelloAgentsLLM,
    system_prompt: String,
}

impl Planner {
    pub fn new(llm: HelloAgentsLLM, system_prompt: Option<String>) -> Self {
        let system_prompt = system_prompt.unwrap_or_else(|| {
            "你是一个顶级的AI规划专家。你的任务是将用户提出的复杂问题分解成一个由多个简单步骤组成的行动计划。\n请确保计划中的每个步骤都是一个独立的、可执行的子任务，并且严格按照逻辑顺序排列。".to_string()
        });
        Self { llm, system_prompt }
    }

    pub fn plan(&self, question: &str) -> Result<Vec<String>, HelloAgentException> {
        println!("--- 正在生成计划 ---");

        let plan_tool = serde_json::json!({
            "type": "function",
            "function": {
                "name": "generate_plan",
                "description": "生成解决问题的分步计划",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "steps": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "按顺序排列的执行步骤列表"
                        }
                    },
                    "required": ["steps"]
                }
            }
        });

        let messages = vec![
            Message::new_text(&self.system_prompt, MessageRole::System),
            Message::new_text(
                &format!("请为以下问题生成详细的执行计划：\n\n{}", question),
                MessageRole::User,
            ),
        ];

        let tools = vec![{
            let mut m = HashMap::new();
            m.insert("type".to_string(), Value::String("function".to_string()));
            m.insert("function".to_string(), plan_tool["function"].clone());
            m
        }];

        // 直接传递 tool_choice 字符串，与 HelloAgentsLLM 签名一致
        let response = self.llm.invoke_with_tools(
            messages,
            tools,
            Some("generate_plan".to_string()),   // 强制调用 generate_plan
        )?;

        if let Some(tc) = response.tool_calls.first() {
            let args_str = tc
                .function
                .as_ref()
                .and_then(|f| f.arguments.clone())
                .unwrap_or_default();
            let args: Value = serde_json::from_str(&args_str).unwrap_or_default();
            let steps: Vec<String> = args["steps"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            println!("✅ 计划已生成:");
            for (i, step) in steps.iter().enumerate() {
                println!("  {}. {}", i + 1, step);
            }
            Ok(steps)
        } else {
            Ok(vec![])
        }
    }
}

// -------------------- Executor --------------------
pub struct Executor {
    llm: HelloAgentsLLM,
    system_prompt: String,
    tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    enable_tool_calling: bool,
    max_tool_iterations: usize,
}

impl Executor {
    pub fn new(
        llm: HelloAgentsLLM,
        system_prompt: Option<String>,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        enable_tool_calling: bool,
        max_tool_iterations: usize,
    ) -> Self {
        let system_prompt = system_prompt.unwrap_or_else(|| {
            "你是一位顶级的AI执行专家。你的任务是严格按照给定的计划，一步步地解决问题。\n请专注于解决当前步骤，并输出该步骤的最终答案。".to_string()
        });
        let enable = enable_tool_calling && tool_registry.is_some();
        Self {
            llm,
            system_prompt,
            tool_registry,
            enable_tool_calling: enable,
            max_tool_iterations,
        }
    }

    pub fn execute(
        &self,
        question: &str,
        plan: &[String],
    ) -> Result<String, HelloAgentException> {
        let mut history: Vec<HashMap<String, String>> = Vec::new();
        let mut final_answer = String::new();

        println!("\n--- 正在执行计划 ---");
        for (i, step) in plan.iter().enumerate() {
            let step_num = i + 1;
            println!(
                "\n-> 正在执行步骤 {}/{}: {}",
                step_num,
                plan.len(),
                step
            );

            let context = format!(
                "# 原始问题:\n{}\n\n# 完整计划:\n{}\n\n# 历史步骤与结果:\n{}\n\n# 当前步骤:\n{}\n\n请执行当前步骤并给出结果。",
                question,
                self.format_plan(plan),
                if history.is_empty() {
                    "无".to_string()
                } else {
                    self.format_history(&history)
                },
                step
            );

            let result = self.execute_step(&context)?;
            let mut record = HashMap::new();
            record.insert("step".to_string(), step.clone());
            record.insert("result".to_string(), result.clone());
            history.push(record);
            final_answer = result.clone();
            println!("✅ 步骤 {} 已完成，结果: {}", step_num, final_answer);
        }

        Ok(final_answer)
    }

    fn format_plan(&self, plan: &[String]) -> String {
        plan.iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_history(&self, history: &[HashMap<String, String>]) -> String {
        history
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let step = h.get("step").cloned().unwrap_or_default();
                let result = h.get("result").cloned().unwrap_or_default();
                format!("步骤 {}: {}\n结果: {}", i + 1, step, result)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn execute_step(&self, context: &str) -> Result<String, HelloAgentException> {
        let mut messages = vec![
            Message::new_text(&self.system_prompt, MessageRole::System),
            Message::new_text(context, MessageRole::User),
        ];

        if !self.enable_tool_calling || self.tool_registry.is_none() {
            // invoke 无第二个参数
            return self.llm.invoke(messages).map(|r| r.content);
        }

        let tool_schemas = self.build_tool_schemas()?;
        for _ in 0..self.max_tool_iterations {
            // 传递 Some("auto") 作为 tool_choice
            let response = self.llm.invoke_with_tools(
                messages.clone(),
                tool_schemas.clone(),
                Some("auto".to_string()),
            )?;

            if response.tool_calls.is_empty() {
                return Ok(response.content.unwrap_or_default());
            }

            messages.push(crate::agents::base::build_assistant_tool_message(&response));

            for tc in &response.tool_calls {
                let tool_name = tc
                    .function
                    .as_ref()
                    .and_then(|f| f.name.clone())
                    .unwrap_or_default();
                let tool_call_id = tc.id.clone().unwrap_or_default();
                let args_str = tc
                    .function
                    .as_ref()
                    .and_then(|f| f.arguments.clone())
                    .unwrap_or_default();

                let arguments = match serde_json::from_str::<Value>(&args_str) {
                    Ok(args) => args,
                    Err(e) => {
                        eprintln!("❌ 参数解析失败: {}", e);
                        messages.push(Message {
                            role: MessageRole::Tool,
                            tool_call_id: Some(tool_call_id.clone()),
                            content: Some(MessageContent::Text(format!(
                                "错误：参数格式不正确 - {}",
                                e
                            ))),
                            ..Default::default()
                        });
                        continue;
                    }
                };

                let result = self.execute_tool_internal(&tool_name, &arguments)?;
                messages.push(Message {
                    role: MessageRole::Tool,
                    tool_call_id: Some(tool_call_id),
                    content: Some(MessageContent::Text(result)),
                    ..Default::default()
                });
            }
        }

        // invoke 无第二个参数
        self.llm.invoke(messages).map(|r| r.content)
    }

    fn build_tool_schemas(&self) -> Result<Vec<HashMap<String, Value>>, HelloAgentException> {
        if let Some(reg) = &self.tool_registry {
            let reg = reg.lock().unwrap();
            let tools = reg.get_all_tools();
            Ok(tools
                .iter()
                .map(|t| t.to_dict().into_iter().map(|(k, v)| (k, v)).collect())
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    fn execute_tool_internal(
        &self,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<String, HelloAgentException> {
        if let Some(reg) = &self.tool_registry {
            let mut reg = reg.lock().unwrap();
            let input = arguments
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let resp = reg.execute_tool(tool_name, input);
            match resp.status {
                ToolStatus::Error => {
                    let code = resp
                        .error_info
                        .as_ref()
                        .map(|e| e.code.as_str())
                        .unwrap_or("UNKNOWN");
                    Ok(format!("❌ 错误 [{}]: {}", code, resp.text))
                }
                ToolStatus::Partial => Ok(format!("⚠️ 部分成功: {}", resp.text)),
                _ => Ok(resp.text),
            }
        } else {
            Err(HelloAgentException::config("未配置工具注册表"))
        }
    }
}

// -------------------- PlanSolveAgent --------------------
pub struct PlanSolveAgent {
    base: AgentBase,
    planner: Planner,
    executor: Executor,
}

impl PlanSolveAgent {
    pub fn new(
        name: &str,
        llm: HelloAgentsLLM,
        system_prompt: Option<String>,
        config: Config,
        planner_prompt: Option<String>,
        executor_prompt: Option<String>,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        enable_tool_calling: bool,
        max_tool_iterations: usize,
    ) -> Self {
        let planner = Planner::new(llm.clone(), planner_prompt);
        let executor = Executor::new(
            llm.clone(),
            executor_prompt,
            tool_registry.clone(),
            enable_tool_calling,
            max_tool_iterations,
        );

        // AgentBase 中的 LLM 不会被直接使用，仅作为占位
        let base = AgentBase::new(
            name,
            Arc::new(crate::infra::llm::adapters::openai::OpenAIAdapter::new("", "", 0, ""))
                as Arc<dyn crate::core::traits::adapter::LLMAdapter>,
            system_prompt,
            config,
            None,
        );

        Self {
            base,
            planner,
            executor,
        }
    }
}

#[async_trait]
impl Agent for PlanSolveAgent {
    type Context = AgentBase;

    fn name(&self) -> &str {
        &self.base.name
    }
    fn context(&self) -> &Self::Context {
        &self.base
    }
    fn context_mut(&mut self) -> &mut Self::Context {
        &mut self.base
    }

    fn run(
        &mut self,
        input_text: &str,
        _kwargs: HashMap<String, String>,
    ) -> Result<String, HelloAgentException> {
        println!("\n🤖 {} 开始处理问题: {}", self.base.name, input_text);

        let plan = self.planner.plan(input_text)?;
        if plan.is_empty() {
            let final_answer = "无法生成有效的行动计划，任务终止。".to_string();
            self.base
                .history
                .append(Message::new_text(input_text, MessageRole::User));
            self.base
                .history
                .append(Message::new_text(&final_answer, MessageRole::Assistant));
            return Ok(final_answer);
        }

        let final_answer = self.executor.execute(input_text, &plan)?;
        println!("\n--- 任务完成 ---\n最终答案: {}", final_answer);

        self.base
            .history
            .append(Message::new_text(input_text, MessageRole::User));
        self.base
            .history
            .append(Message::new_text(&final_answer, MessageRole::Assistant));

        Ok(final_answer)
    }

    fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>> {
        Vec::new()
    }

    fn execute_tool_call(&self, _tool_name: &str, _arguments: Value) -> String {
        "".to_string()
    }

    fn run_as_subagent(
        &mut self,
        task: &str,
        _tool_filter: Option<Box<dyn ToolFilter>>,
        _return_summary: bool,
        _max_steps: Option<usize>,
    ) -> Result<SubagentResult, HelloAgentException> {
        let result = self.run(task, HashMap::new())?;
        Ok(SubagentResult {
            success: true,
            summary: result.clone(),
            metadata: SubagentMetadata {
                steps: 1,
                tokens: 0,
                duration_seconds: 0.0,
                tools_used: vec![],
                error: None,
            },
        })
    }
}