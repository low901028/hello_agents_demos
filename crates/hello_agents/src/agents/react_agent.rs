//! react_agent.rs
//! ReAct Agent - 基于 Function Calling 的推理与行动

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;

use crate::core::llm_agent::AgentBase;
use crate::core::agent_trait::{Agent, SubagentMetadata, SubagentResult};
use crate::core::config::Config;
use crate::core::exceptions::HelloAgentException;
use crate::core::hello_agents_llm::HelloAgentsLLM;
use crate::core::lifecycle::{AgentEvent, EventType, LifecycleHook};
use crate::core::llm_resp_req::{Message, MessageContent, MessageRole};
use crate::tools::tool_base::Tool;
use crate::tools::tool_filter::ToolFilter;
use crate::tools::tool_registry::ToolRegistry;
use crate::tools::tool_response::{ToolResponse, ToolStatus};

/// 默认的系统提示词
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

/// ReAct Agent - 基于 Function Calling 的推理与行动
///
/// 核心改进：
/// - 使用 OpenAI Function Calling（结构化输出）
/// - 支持 Thought 工具（显式推理）
/// - 支持 Finish 工具（结束流程）
/// - 无需正则解析，解析成功率 99%+
pub struct ReActAgent {
    base: AgentBase,
    max_steps: usize,
    /// 内置工具名称集合
    builtin_tools: HashSet<String>,
    // 当前步数（用于异常保存）
    current_step: usize,
    total_tokens: usize,
}

impl ReActAgent {
    pub fn new(
        name: &str,
        llm: HelloAgentsLLM,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        system_prompt: Option<String>,
        config: Option<Config>,
        max_steps: usize,
    ) -> Self {
        let system_prompt = system_prompt.unwrap_or_else(|| DEFAULT_REACT_SYSTEM_PROMPT.to_string());
        let tool_registry = tool_registry
            .unwrap_or_else(|| Arc::new(Mutex::new(ToolRegistry::new(None))));
        let working_dir = std::env::current_dir().unwrap_or_default();

        let mut builtin = HashSet::new();
        builtin.insert("Thought".to_string());
        builtin.insert("Finish".to_string());

        Self {
            base: AgentBase::new(
                name,
                llm,
                Some(system_prompt),
                config,
                Some(tool_registry),
                Some(working_dir),
            ),
            max_steps,
            builtin_tools: builtin,
            current_step: 0,
            total_tokens: 0,
        }
    }

    /// 添加工具到工具注册表
    pub fn add_tool(&mut self, tool: Box<dyn Tool>, auto_expand: bool) {
        if let Some(ref reg) = self.base.tool_registry {
            let mut reg = reg.lock().unwrap();
            reg.register_tool(tool, auto_expand);
        }
    }

    /// 处理内置工具调用
    fn handle_builtin_tool(&self, tool_name: &str, arguments: &Value) -> (String, bool, Option<String>) {
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
}

#[async_trait]
impl Agent for ReActAgent {
    fn name(&self) -> &str {
        &self.base.name
    }

    fn run(&mut self, input_text: &str, kwargs: HashMap<String, String>) -> Result<String, HelloAgentException> {
        let session_start = std::time::Instant::now();
        self.current_step = 0;
        self.total_tokens = 0;

        // 构建消息列表
        let mut messages = self.build_messages(input_text);

        // 构建工具 schemas（包含内置工具和用户工具）
        let tool_schemas = self.build_tool_schemas();

        // 记录用户消息（trace_logger 简化处理）
        if let Some(ref mut logger) = self.base.trace_logger {
            logger.log_event(
                "message_written",
                serde_json::json!({"role": "user", "content": input_text}),
                None,
            );
        }

        println!("\n🤖 {} 开始处理问题: {}", self.base.name, input_text);

        while self.current_step < self.max_steps {
            self.current_step += 1;
            println!("\n--- 第 {} 步 ---", self.current_step);

            // 调用 LLM（Function Calling）
            let response = match self.base.llm.invoke_with_tools(
                messages.clone(),
                tool_schemas.clone(),
                Some("auto".to_string()),
            ) {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("❌ LLM 调用失败: {}", e);
                    if let Some(ref mut logger) = self.base.trace_logger {
                        logger.log_event(
                            "error",
                            serde_json::json!({"error_type": "LLM_ERROR", "message": e.to_string()}),
                            Some(self.current_step),
                        );
                    }
                    break;
                }
            };

            // 累计 tokens
            self.total_tokens += response.usage.total_tokens as usize;

            // 记录模型输出
            if let Some(ref mut logger) = self.base.trace_logger {
                logger.log_event(
                    "model_output",
                    serde_json::json!({
                        "content": response.content,
                        "tool_calls": response.tool_calls.len(),
                        "usage": {
                            "total_tokens": response.usage.total_tokens,
                            "cost": 0.0
                        }
                    }),
                    Some(self.current_step),
                );
            }

            // 处理工具调用
            let tool_calls = response.tool_calls.clone();
            if tool_calls.is_empty() {
                // 没有工具调用，直接返回文本响应
                let final_answer = response.content.unwrap_or_else(|| "抱歉，我无法回答这个问题。".to_string());
                println!("💬 直接回复: {}", final_answer);

                // 保存到历史记录
                self.base.history_manager.append(Message::new_text(input_text, MessageRole::User));
                self.base.history_manager.append(Message::new_text(&final_answer, MessageRole::Assistant));

                if let Some(ref mut logger) = self.base.trace_logger {
                    let duration = session_start.elapsed().as_secs_f64();
                    logger.log_event(
                        "session_end",
                        serde_json::json!({
                            "duration": duration,
                            "total_steps": self.current_step,
                            "final_answer": final_answer,
                            "status": "success"
                        }),
                        None,
                    );
                    logger.finalize();
                }

                return Ok(final_answer);
            }

            // 将助手消息添加到历史
            let assistant_msg = Message {
                role: MessageRole::Assistant,
                content: response.content.map(MessageContent::Text),
                tool_calls: Some(
                    tool_calls.iter().map(|tc| crate::core::llm_resp_req::ToolCall {
                        id: tc.id.clone(),
                        call_type: Some("function".to_string()),
                        function: Some(crate::core::llm_resp_req::FunctionCall {
                            name: tc.function.as_ref().and_then(|f| f.name.clone()),
                            arguments: tc.function.as_ref().and_then(|f| f.arguments.clone()),
                        }),
                        ..Default::default()
                    }).collect(),
                ),
                ..Default::default()
            };
            messages.push(assistant_msg);

            // 执行所有工具调用
            for tool_call in &tool_calls {
                let tool_name = tool_call.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default();
                let tool_call_id = tool_call.id.clone().unwrap_or_default();

                // 解析参数（JSON 字符串 → Value）
                let args_str = tool_call.function.as_ref().and_then(|f| f.arguments.clone()).unwrap_or_default();
                let arguments = match serde_json::from_str::<Value>(&args_str) {
                    Ok(args) => args,
                    Err(e) => {
                        eprintln!("❌ 工具参数解析失败: {}", e);
                        messages.push(Message {
                            role: MessageRole::Tool,
                            tool_call_id: Some(tool_call_id.clone()),
                            content: Some(MessageContent::Text(format!("错误：参数格式不正确 - {}", e))),
                            ..Default::default()
                        });
                        continue;
                    }
                };

                // 记录工具调用
                if let Some(ref mut logger) = self.base.trace_logger {
                    logger.log_event(
                        "tool_call",
                        serde_json::json!({
                            "tool_name": &tool_name,
                            "tool_call_id": &tool_call_id,
                            "args": &arguments,
                        }),
                        Some(self.current_step),
                    );
                }

                // 检查是否是内置工具
                if self.builtin_tools.contains(&tool_name) {
                    let (content, finished, final_answer) = self.handle_builtin_tool(&tool_name, &arguments);
                    println!("🔧 {}: {}", tool_name, content);

                    // 记录工具结果
                    if let Some(ref mut logger) = self.base.trace_logger {
                        logger.log_event(
                            "tool_result",
                            serde_json::json!({
                                "tool_name": &tool_name,
                                "tool_call_id": &tool_call_id,
                                "status": "success",
                                "result": &content,
                            }),
                            Some(self.current_step),
                        );
                    }

                    // 检查是否是 Finish
                    if tool_name == "Finish" && finished {
                        if let Some(answer) = final_answer {
                            println!("🎉 最终答案: {}", answer);

                            self.base.history_manager.append(Message::new_text(input_text, MessageRole::User));
                            self.base.history_manager.append(Message::new_text(&answer, MessageRole::Assistant));

                            if let Some(ref mut logger) = self.base.trace_logger {
                                let duration = session_start.elapsed().as_secs_f64();
                                logger.log_event(
                                    "session_end",
                                    serde_json::json!({
                                        "duration": duration,
                                        "total_steps": self.current_step,
                                        "final_answer": answer,
                                        "status": "success"
                                    }),
                                    None,
                                );
                                logger.finalize();
                            }

                            return Ok(answer);
                        }
                    }

                    // 添加工具结果到消息
                    messages.push(Message {
                        role: MessageRole::Tool,
                        tool_call_id: Some(tool_call_id),
                        content: Some(MessageContent::Text(content)),
                        ..Default::default()
                    });
                } else {
                    // 用户工具
                    println!("🎬 调用工具: {}({})", tool_name, arguments);

                    // 执行工具（复用基类方法）
                    let result = self.execute_tool_call(&tool_name, arguments.clone());

                    // 记录工具结果
                    if let Some(ref mut logger) = self.base.trace_logger {
                        logger.log_event(
                            "tool_result",
                            serde_json::json!({
                                "tool_name": &tool_name,
                                "tool_call_id": &tool_call_id,
                                "result": &result,
                            }),
                            Some(self.current_step),
                        );
                    }

                    if result.starts_with("❌") {
                        println!("{}", result);
                    } else {
                        println!("👀 观察: {}", result);
                    }

                    // 添加工具结果到消息
                    messages.push(Message {
                        role: MessageRole::Tool,
                        tool_call_id: Some(tool_call_id),
                        content: Some(MessageContent::Text(result)),
                        ..Default::default()
                    });
                }
            }
        }

        // 达到最大步数
        println!("⏰ 已达到最大步数，流程终止。");
        let final_answer = "抱歉，我无法在限定步数内完成这个任务。".to_string();

        self.base.history_manager.append(Message::new_text(input_text, MessageRole::User));
        self.base.history_manager.append(Message::new_text(&final_answer, MessageRole::Assistant));

        if let Some(ref mut logger) = self.base.trace_logger {
            let duration = session_start.elapsed().as_secs_f64();
            logger.log_event(
                "session_end",
                serde_json::json!({
                    "duration": duration,
                    "total_steps": self.current_step,
                    "final_answer": final_answer,
                    "status": "timeout"
                }),
                None,
            );
            logger.finalize();
        }

        Ok(final_answer)
    }

    fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>> {
        let mut schemas = Vec::new();

        // 1. 内置工具：Thought
        schemas.push({
            let mut map = HashMap::new();
            map.insert("type".to_string(), Value::String("function".to_string()));
            let func = serde_json::json!({
                "name": "Thought",
                "description": "分析问题，制定策略，记录推理过程。在需要思考时调用此工具。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "reasoning": {
                            "type": "string",
                            "description": "你的推理过程和分析"
                        }
                    },
                    "required": ["reasoning"]
                }
            });
            map.insert("function".to_string(), func);
            map
        });

        // 2. 内置工具：Finish
        schemas.push({
            let mut map = HashMap::new();
            map.insert("type".to_string(), Value::String("function".to_string()));
            let func = serde_json::json!({
                "name": "Finish",
                "description": "当你有足够信息得出结论时，使用此工具返回最终答案。",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "answer": {
                            "type": "string",
                            "description": "最终答案"
                        }
                    },
                    "required": ["answer"]
                }
            });
            map.insert("function".to_string(), func);
            map
        });

        // 3. 用户工具（从注册表构建）
        if let Some(ref reg) = self.base.tool_registry {
            let reg = reg.lock().unwrap();
            let tools = reg.get_all_tools();
            for tool in tools {
                let mut properties = serde_json::Map::new();
                let mut required = Vec::new();
                for param in tool.get_parameters() {
                    let mut prop = serde_json::Map::new();
                    prop.insert("type".into(), Value::String(param.param_type));
                    prop.insert("description".into(), Value::String(param.description));
                    if let Some(default) = param.default {
                        prop.insert("default".into(), default);
                    }
                    let param_name = param.name;
                    properties.insert(param_name.clone().to_owned(), Value::Object(prop));
                    if param.required {
                        required.push(param_name);
                    }
                }
                let mut func = serde_json::Map::new();
                func.insert("name".into(), Value::String(tool.name().to_string()));
                func.insert("description".into(), Value::String(tool.base().description.to_string()));
                let mut params = serde_json::Map::new();
                params.insert("type".into(), Value::String("object".into()));
                params.insert("properties".into(), Value::Object(properties));
                if !required.is_empty() {
                    params.insert("required".into(), Value::Array(required.into_iter().map(Value::String).collect()));
                }
                func.insert("parameters".into(), Value::Object(params));
                let mut schema = serde_json::Map::new();
                schema.insert("type".into(), Value::String("function".into()));
                schema.insert("function".into(), Value::Object(func));
                schemas.push(schema.into_iter().collect());
            }
        }

        schemas
    }

    fn execute_tool_call(&self, tool_name: &str, arguments: Value) -> String {
        if let Some(ref reg) = self.base.tool_registry {
            let mut reg = reg.lock().unwrap();
            let input_str = arguments.get("input").and_then(|v| v.as_str()).unwrap_or("");
            let resp = reg.execute_tool(tool_name, input_str);
            match resp.status {
                ToolStatus::Error => {
                    let code = resp.error_info.as_ref().map(|e| e.code.as_str()).unwrap_or("UNKNOWN");
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
        tool_filter: Option<Box<dyn ToolFilter>>,
        return_summary: bool,
        max_steps_override: Option<usize>,
    ) -> Result<SubagentResult, HelloAgentException> {
        // 简单实现：直接调用 run
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

impl ReActAgent {
    /// 构建消息列表（仅包含 system prompt 和用户问题）
    fn build_messages(&self, input_text: &str) -> Vec<Message> {
        let mut messages = Vec::new();
        if let Some(ref sp) = self.base.system_prompt {
            messages.push(Message::new_text(sp, MessageRole::System));
        }
        messages.push(Message::new_text(input_text, MessageRole::User));
        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_tool_handling() {
        let llm = unimplemented!("需要真实的 HelloAgentsLLM");
        let agent = ReActAgent::new(
            "test",
            llm,
            None,
            None,
            None,
            5,
        );
        // 测试 Thought
        let args = serde_json::json!({"reasoning": "我需要分析"});
        let (content, finished, _) = agent.handle_builtin_tool("Thought", &args);
        assert!(content.contains("推理:"));
        assert!(!finished);

        // 测试 Finish
        let args = serde_json::json!({"answer": "42"});
        let (content, finished, final_ans) = agent.handle_builtin_tool("Finish", &args);
        assert!(content.contains("最终答案:"));
        assert!(finished);
        assert_eq!(final_ans, Some("42".to_string()));
    }
}