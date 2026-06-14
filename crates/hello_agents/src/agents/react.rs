use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;

use crate::agents::base::{AgentBase, ToolCallRunner};
use crate::core::traits::agent::{Agent, SubagentMetadata, SubagentResult};
use crate::core::traits::context::AgentContext;
use crate::core::types::config::Config;
use crate::core::types::exceptions::HelloAgentException;
use crate::core::types::message::{Message, MessageContent, MessageRole};
use crate::tools::filter::ToolFilter;
use crate::tools::registry::ToolRegistry;

const DEFAULT_REACT_SYSTEM_PROMPT: &str = "你是一个具备推理和行动能力的 AI 助手。

## 工作流程
你可以通过调用工具来完成任务：

1. **Thought 工具**：用于记录你的推理过程和分析
   - 在需要思考时调用
   - 参数：reasoning（你的推理内容）

2. **业务工具**：用于获取信息或执行操作
   - 根据任务需求选择合适的工具
   - 可以多次调用不同工具

3. **Finish 工具**：用于返回最终答案
   - 当你有足够信息得出结论时调用
   - 参数：answer（最终答案）

## 重要提醒
- 主动使用 Thought 工具记录推理过程
- 可以多次调用工具获取信息
- 只有在确信有足够信息时才调用 Finish";

pub struct ReActAgent {
    base: AgentBase,
    max_steps: usize,
    builtin_tools: HashSet<String>,
    current_step: usize,
    total_tokens: usize,
    runner: ToolCallRunner<AgentBase>,   // 用于执行业务工具
}

impl ReActAgent {
    pub fn new(
        name: &str,
        llm: Arc<dyn crate::core::traits::adapter::LLMAdapter>,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        system_prompt: Option<String>,
        config: Config,
        max_steps: usize,
    ) -> Self {
        let system_prompt =
            system_prompt.unwrap_or_else(|| DEFAULT_REACT_SYSTEM_PROMPT.to_string());
        let mut builtin = HashSet::new();
        builtin.insert("Thought".to_string());
        builtin.insert("Finish".to_string());

        // 确保 tool_registry 至少存在一个空注册表，方便 add_tool 使用
        let tool_registry = Some(tool_registry.unwrap_or(Arc::new(Mutex::new(ToolRegistry::new(None)))));
        let base = AgentBase::new(name, llm, Some(system_prompt), config, tool_registry);
        let runner = ToolCallRunner::new(Arc::new(base.clone()), max_steps);

        Self {
            base,
            max_steps,
            builtin_tools: builtin,
            current_step: 0,
            total_tokens: 0,
            runner,
        }
    }

    pub fn add_tool(
        &mut self,
        tool: Box<dyn crate::core::traits::tool::Tool>,
        auto_expand: bool,
    ) {
        if let Some(reg) = self.base.tool_registry_arc() {
            reg.lock().unwrap().register_tool(tool, auto_expand);
        }
    }

    fn handle_builtin_tool(
        &self,
        tool_name: &str,
        arguments: &Value,
    ) -> (String, bool, Option<String>) {
        match tool_name {
            "Thought" => {
                let reasoning = arguments["reasoning"].as_str().unwrap_or("");
                (format!("推理: {}", reasoning), false, None)
            }
            "Finish" => {
                let answer = arguments["answer"].as_str().unwrap_or("").to_string();
                (format!("最终答案: {}", answer), true, Some(answer))
            }
            _ => (format!("未知的内置工具: {}", tool_name), false, None),
        }
    }

    fn build_messages(&self, input_text: &str) -> Vec<Message> {
        let mut messages = Vec::new();
        if let Some(ref sp) = self.base.system_prompt {
            messages.push(Message::new_text(sp, MessageRole::System));
        }
        messages.push(Message::new_text(input_text, MessageRole::User));
        messages
    }
}

#[async_trait]
impl Agent for ReActAgent {
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
        self.current_step = 0;
        self.total_tokens = 0;

        let mut messages = self.build_messages(input_text);
        let tool_schemas = self.build_tool_schemas();

        println!("\n🤖 {} 开始处理问题: {}", self.base.name, input_text);

        while self.current_step < self.max_steps {
            self.current_step += 1;
            println!("\n--- 第 {} 步 ---", self.current_step);

            let response = match self.base.llm.invoke_with_tools(
                messages.clone(),
                tool_schemas.clone(),
                HashMap::new(),
            ) {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("❌ LLM 调用失败: {}", e);
                    break;
                }
            };

            self.total_tokens += response.usage.total_tokens as usize;

            let tool_calls = response.tool_calls.clone();
            if tool_calls.is_empty() {
                let final_answer = response
                    .content
                    .unwrap_or_else(|| "抱歉，我无法回答这个问题。".to_string());
                println!("💬 直接回复: {}", final_answer);

                self.base
                    .history
                    .append(Message::new_text(input_text, MessageRole::User));
                self.base
                    .history
                    .append(Message::new_text(&final_answer, MessageRole::Assistant));
                return Ok(final_answer);
            }

            messages.push(crate::agents::base::build_assistant_tool_message(&response));

            // 先处理内置工具（Thought/Finish），再处理业务工具
            let mut business_tool_calls = Vec::new();
            for tc in &tool_calls {
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
                        eprintln!("❌ 工具参数解析失败: {}", e);
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

                if self.builtin_tools.contains(&tool_name) {
                    let (content, finished, final_answer) =
                        self.handle_builtin_tool(&tool_name, &arguments);
                    println!("🔧 {}: {}", tool_name, content);

                    if tool_name == "Finish" && finished {
                        if let Some(answer) = final_answer {
                            println!("🎉 最终答案: {}", answer);

                            self.base
                                .history
                                .append(Message::new_text(input_text, MessageRole::User));
                            self.base
                                .history
                                .append(Message::new_text(&answer, MessageRole::Assistant));
                            return Ok(answer);
                        }
                    }

                    messages.push(Message {
                        role: MessageRole::Tool,
                        tool_call_id: Some(tool_call_id),
                        content: Some(MessageContent::Text(content)),
                        ..Default::default()
                    });
                } else {
                    business_tool_calls.push((tool_call_id, tool_name, arguments));
                }
            }

            // 执行业务工具，使用 ToolCallRunner
            if !business_tool_calls.is_empty() {
                // 将业务工具调用的结果逐一处理
                for (tool_call_id, tool_name, arguments) in business_tool_calls {
                    println!("🎬 调用工具: {}({})", tool_name, arguments);
                    let result = self.base.execute_tool_call(&tool_name, &arguments)
                        .unwrap_or_else(|e| format!("❌ {}", e));
                    if result.starts_with("❌") {
                        println!("{}", result);
                    } else {
                        println!("👀 观察: {}", result);
                    }
                    messages.push(Message {
                        role: MessageRole::Tool,
                        tool_call_id: Some(tool_call_id),
                        content: Some(MessageContent::Text(result)),
                        ..Default::default()
                    });
                }
                // 继续循环，让模型基于工具结果继续推理
            }
        }

        let final_answer = "抱歉，我无法在限定步数内完成这个任务。".to_string();
        self.base
            .history
            .append(Message::new_text(input_text, MessageRole::User));
        self.base
            .history
            .append(Message::new_text(&final_answer, MessageRole::Assistant));
        Ok(final_answer)
    }

    fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>> {
        let mut schemas = Vec::new();

        // 内置 Thought 工具
        schemas.push({
            let mut map = HashMap::new();
            map.insert("type".into(), "function".into());
            let func = serde_json::json!({
                "name": "Thought",
                "description": "分析问题，制定策略，记录推理过程。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "reasoning": {"type": "string", "description": "推理过程"}
                    },
                    "required": ["reasoning"]
                }
            });
            map.insert("function".into(), func);
            map
        });

        // 内置 Finish 工具
        schemas.push({
            let mut map = HashMap::new();
            map.insert("type".into(), "function".into());
            let func = serde_json::json!({
                "name": "Finish",
                "description": "返回最终答案",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "answer": {"type": "string", "description": "最终答案"}
                    },
                    "required": ["answer"]
                }
            });
            map.insert("function".into(), func);
            map
        });

        // 用户注册的工具（使用 to_openai_schema 生成标准结构）
        if let Some(reg_arc) = self.base.tool_registry_arc() {
            let reg = reg_arc.lock().unwrap();
            for tool in reg.get_all_tools() {
                let name = tool.name().to_string();
                if self.builtin_tools.contains(&name) {
                    continue;
                }
                let schema_val = tool.to_openai_schema();
                if let Value::Object(map) = schema_val {
                    schemas.push(map.into_iter().collect());
                }
            }
        }

        schemas
    }

    fn execute_tool_call(&self, tool_name: &str, arguments: Value) -> String {
        // 委托给 AgentContext 实现，保证注册表访问统一
        self.base.execute_tool_call(tool_name, &arguments)
            .unwrap_or_else(|e| format!("❌ {}", e))
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
                steps: self.current_step,
                tokens: self.total_tokens,
                duration_seconds: 0.0,
                tools_used: vec![],
                error: None,
            },
        })
    }
}

impl crate::tools::builtin::task::SubagentExecutor for ReActAgent {
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