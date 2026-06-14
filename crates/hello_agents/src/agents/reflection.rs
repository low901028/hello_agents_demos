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
use crate::tools::filter::ToolFilter;
use crate::tools::registry::ToolRegistry;
use crate::tools::response::ToolStatus;

#[derive(Clone)]
pub struct Memory {
    records: Vec<MemoryRecord>,
}

#[derive(Clone)]
struct MemoryRecord {
    record_type: String,
    content: String,
}

impl Memory {
    pub fn new() -> Self {
        Self { records: vec![] }
    }

    pub fn add_record(&mut self, record_type: &str, content: &str) {
        self.records.push(MemoryRecord {
            record_type: record_type.into(),
            content: content.into(),
        });
        println!("📝 记忆已更新，新增一条 '{}' 记录。", record_type);
    }

    pub fn get_last_execution(&self) -> String {
        self.records
            .iter()
            .rev()
            .find(|r| r.record_type == "execution")
            .map(|r| r.content.clone())
            .unwrap_or_default()
    }
}

pub struct ReflectionAgent {
    base: AgentBase,
    max_iterations: usize,
    memory: Memory,
    enable_tool_calling: bool,
    max_tool_iterations: usize,
}

impl ReflectionAgent {
    pub fn new(
        name: &str,
        llm: Arc<dyn crate::core::traits::adapter::LLMAdapter>,
        system_prompt: Option<String>,
        config: Config,
        max_iterations: usize,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        enable_tool_calling: bool,
        max_tool_iterations: usize,
    ) -> Self {
        let default_prompt = "你是一个具有自我反思能力的AI助手。你的工作流程是：\n\
            1. 首先尝试完成用户的任务\n\
            2. 然后反思你的回答，找出可能的问题或改进空间\n\
            3. 根据反思结果优化你的回答\n\
            4. 如果回答已经很好，在反思时回复\"无需改进\"\n\n\
            请始终保持批判性思维，追求更高质量的输出。".to_string();
        let system_prompt = system_prompt.unwrap_or(default_prompt);
        let enable = enable_tool_calling && tool_registry.is_some();

        let tool_registry = Some(tool_registry.unwrap_or(Arc::new(Mutex::new(ToolRegistry::new(None)))));
        Self {
            base: AgentBase::new(name, llm, Some(system_prompt), config, tool_registry),
            max_iterations,
            memory: Memory::new(),
            enable_tool_calling: enable,
            max_tool_iterations,
        }
    }

    fn get_llm_response(
        &self,
        messages: Vec<Message>,
        _kwargs: Option<HashMap<String, String>>,
    ) -> Result<String, HelloAgentException> {
        if !self.enable_tool_calling || self.base.tool_registry.is_none() {
            return self.base.llm.invoke(messages, HashMap::new()).map(|r| r.content);
        }

        let tool_schemas = self.build_tool_schemas();
        let mut current = messages;
        for _ in 0..self.max_tool_iterations {
            let response = self.base.llm.invoke_with_tools(
                current.clone(),
                tool_schemas.clone(),
                {
                    let mut m = HashMap::new();
                    m.insert(
                        "tool_choice".to_string(),
                        Value::String("auto".to_string()),
                    );
                    m
                },
            )?;

            if response.tool_calls.is_empty() {
                return Ok(response.content.unwrap_or_default());
            }

            current.push(crate::agents::base::build_assistant_tool_message(&response));

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
                        current.push(Message {
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

                let result = self.execute_tool_call(&tool_name, arguments);
                current.push(Message {
                    role: MessageRole::Tool,
                    tool_call_id: Some(tool_call_id),
                    content: Some(MessageContent::Text(result)),
                    ..Default::default()
                });
            }
        }

        self.base.llm.invoke(current, HashMap::new()).map(|r| r.content)
    }

    fn execute_task(
        &self,
        task: &str,
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<String, HelloAgentException> {
        let messages = vec![
            Message::new_text(
                self.base.system_prompt.as_deref().unwrap_or(""),
                MessageRole::System,
            ),
            Message::new_text(&format!("请完成以下任务：\n\n{}", task), MessageRole::User),
        ];
        self.get_llm_response(messages, kwargs)
    }

    fn reflect_on_result(
        &self,
        task: &str,
        result: &str,
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<String, HelloAgentException> {
        let messages = vec![
            Message::new_text(
                self.base.system_prompt.as_deref().unwrap_or(""),
                MessageRole::System,
            ),
            Message::new_text(
                &format!(
                    "请仔细审查以下回答，并找出可能的问题或改进空间：\n\n# 原始任务:\n{}\n\n# 当前回答:\n{}\n\n请分析这个回答的质量，指出不足之处，并提出具体的改进建议。\n如果回答已经很好，请回答\"无需改进\"。",
                    task, result
                ),
                MessageRole::User,
            ),
        ];
        self.get_llm_response(messages, kwargs)
    }

    fn refine_result(
        &self,
        task: &str,
        last_attempt: &str,
        feedback: &str,
        kwargs: Option<HashMap<String, String>>,
    ) -> Result<String, HelloAgentException> {
        let messages = vec![
            Message::new_text(
                self.base.system_prompt.as_deref().unwrap_or(""),
                MessageRole::System,
            ),
            Message::new_text(
                &format!(
                    "请根据反馈意见改进你的回答：\n\n# 原始任务:\n{}\n\n# 上一轮回答:\n{}\n\n# 反馈意见:\n{}\n\n请提供一个改进后的回答。",
                    task, last_attempt, feedback
                ),
                MessageRole::User,
            ),
        ];
        self.get_llm_response(messages, kwargs)
    }
}

#[async_trait]
impl Agent for ReflectionAgent {
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
        println!("\n🤖 {} 开始处理任务: {}", self.base.name, input_text);
        self.memory = Memory::new();

        // 1. 初始执行
        println!("\n--- 正在进行初始尝试 ---");
        let initial = self.execute_task(input_text, None)?;
        self.memory.add_record("execution", &initial);

        // 2. 反思迭代
        for i in 0..self.max_iterations {
            println!("\n--- 第 {}/{} 轮迭代 ---", i + 1, self.max_iterations);

            let last = self.memory.get_last_execution();
            let feedback = self.reflect_on_result(input_text, &last, None)?;
            self.memory.add_record("reflection", &feedback);

            if feedback.contains("无需改进")
                || feedback.to_lowercase().contains("no need for improvement")
            {
                println!("\n✅ 反思认为结果已无需改进，任务完成。");
                break;
            }

            let refined = self.refine_result(input_text, &last, &feedback, None)?;
            self.memory.add_record("execution", &refined);
        }

        let final_answer = self.memory.get_last_execution();
        println!("\n--- 任务完成 ---\n最终结果:\n{}", final_answer);

        self.base
            .history
            .append(Message::new_text(input_text, MessageRole::User));
        self.base
            .history
            .append(Message::new_text(&final_answer, MessageRole::Assistant));
        Ok(final_answer)
    }

    fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>> {
        if let Some(registry) = self.base.tool_registry() {
            registry
                .get_all_tools()
                .iter()
                .map(|t| t.to_dict().into_iter().map(|(k, v)| (k, v)).collect())
                .collect()
        } else {
            Vec::new()
        }
    }

    fn execute_tool_call(&self, tool_name: &str, arguments: Value) -> String {
        if let Some(registry) = self.base.tool_registry() {
            let input = arguments
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let resp = registry.execute_tool(tool_name, input);
            match resp.status {
                ToolStatus::Error => {
                    let code = resp
                        .error_info
                        .as_ref()
                        .map(|e| e.code.as_str())
                        .unwrap_or("UNKNOWN");
                    format!("❌ 错误 [{}]: {}", code, resp.text)
                }
                ToolStatus::Partial => format!("⚠️ 部分成功: {}", resp.text),
                _ => resp.text,
            }
        } else {
            "❌ 错误：未配置工具注册表".to_string()
        }
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

impl crate::tools::builtin::task::SubagentExecutor for ReflectionAgent {
    fn run_as_subagent(
        &mut self,
        task: &str,
        tool_filter: Option<Box<dyn ToolFilter>>,
        return_summary: bool,
        max_steps_override: Option<usize>,
    ) -> Result<SubagentResult, HelloAgentException> {
        <Self as Agent>::run_as_subagent(self, task, tool_filter, return_summary, max_steps_override)
    }
}