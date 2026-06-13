//! simple_agent.rs
//! 简单 Agent 实现，基于 Function Calling

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use serde_json::Value;

use crate::core::llm_agent::AgentBase;
use crate::core::agent_trait::{Agent, SubagentMetadata, SubagentResult};
use crate::core::config::Config;
use crate::core::exceptions::HelloAgentException;
use crate::core::hello_agents_llm::HelloAgentsLLM;
use crate::core::lifecycle::{AgentEvent, EventType, LifecycleHook};
use crate::core::llm_resp_req::{Message, MessageContent, MessageRole};
use crate::observability::trace_logger::TraceLogger;
use crate::tools::tool_base::Tool;
use crate::tools::tool_filter::ToolFilter;
use crate::tools::tool_registry::ToolRegistry;
use crate::tools::tool_response::ToolStatus;

pub struct SimpleAgent {
    base: AgentBase,
    enable_tool_calling: bool,
    max_tool_iterations: usize,
}

impl SimpleAgent {
    pub fn new(
        name: &str,
        llm: HelloAgentsLLM,
        system_prompt: Option<String>,
        config: Option<Config>,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        enable_tool_calling: bool,
        max_tool_iterations: usize,
    ) -> Self {
        let enable = enable_tool_calling && tool_registry.is_some();
        let working_dir = std::env::current_dir().unwrap_or_default();
        Self {
            base: AgentBase::new(name, llm, system_prompt, config, tool_registry, Some(working_dir)),
            enable_tool_calling: enable,
            max_tool_iterations,
        }
    }

    /// 添加工具到 Agent（便利方法）
    ///
    /// # Arguments
    /// * `tool` - Tool 对象
    /// * `auto_expand` - 是否自动展开可展开的工具（默认 true）
    pub fn add_tool(&mut self, tool: Box<dyn Tool>, auto_expand: bool) {
        if self.base.tool_registry.is_none() {
            self.base.tool_registry = Some(Arc::new(Mutex::new(ToolRegistry::new(None))));
            self.enable_tool_calling = true;
        }
        if let Some(ref reg) = self.base.tool_registry {
            let mut reg = reg.lock().unwrap();
            reg.register_tool(tool, auto_expand);
        }
    }

    /// 移除工具（便利方法）
    pub fn remove_tool(&self, tool_name: &str) -> bool {
        if let Some(ref reg) = self.base.tool_registry {
            let mut reg = reg.lock().unwrap();
            reg.unregister(tool_name);
            true
        } else {
            false
        }
    }

    /// 列出所有可用工具
    pub fn list_tools(&self) -> Vec<String> {
        if let Some(ref reg) = self.base.tool_registry {
            let reg = reg.lock().unwrap();
            reg.list_tools()
        } else {
            Vec::new()
        }
    }

    /// 检查是否有可用工具
    pub fn has_tools(&self) -> bool {
        self.enable_tool_calling && self.base.tool_registry.is_some()
    }

    /// 构建消息列表
    fn build_messages(&self, input_text: &str) -> Vec<Message> {
        let mut messages = Vec::new();

        // 添加系统提示词
        if let Some(ref sp) = self.base.system_prompt {
            messages.push(Message::new_text(sp, MessageRole::System));
        }

        // 添加历史消息
        let history = self.base.history_manager.get_history();
        for msg in history {
            messages.push(msg);
        }

        // 添加用户问题
        messages.push(Message::new_text(input_text, MessageRole::User));

        messages
    }
}

#[async_trait]
impl Agent for SimpleAgent {
    fn name(&self) -> &str { &self.base.name }

    fn run(&mut self, input_text: &str, kwargs: HashMap<String, String>) -> Result<String, HelloAgentException> {
        let session_start = Instant::now();

        // 为每次 run 创建新的 TraceLogger（避免多轮对话时文件已关闭的问题）
        let mut trace_logger = if self.base.config.trace_enabled {
            let mut logger = TraceLogger::new(
                &self.base.config.trace_dir,
                self.base.config.trace_sanitize,
                Some(self.base.config.trace_html_include_raw_response),
            );
            logger.log_event(
                "session_start",
                serde_json::json!({
                    "agent_name": self.base.name,
                    "agent_type": "SimpleAgent",
                }),
                None,
            );
            Some(logger)
        } else {
            None
        };

        // 构建消息列表
        let mut messages = self.build_messages(input_text);

        // 记录用户消息
        if let Some(ref mut logger) = trace_logger {
            logger.log_event(
                "message_written",
                serde_json::json!({"role": "user", "content": input_text}),
                None,
            );
        }

        // 如果没有启用工具调用，直接返回 LLM 响应
        if !self.enable_tool_calling || self.base.tool_registry.is_none() {
            let llm_response = self.base.llm.invoke(messages)?;
            let response_text = llm_response.content.clone();

            // 保存到历史记录
            self.base.history_manager.append(Message::new_text(input_text, MessageRole::User));
            self.base.history_manager.append(Message::new_text(&response_text, MessageRole::Assistant));

            if let Some(ref mut logger) = trace_logger {
                let duration = session_start.elapsed().as_secs_f64();
                logger.log_event(
                    "session_end",
                    serde_json::json!({
                        "duration": duration,
                        "final_answer": response_text,
                        "status": "success",
                        "usage": llm_response.usage,
                        "latency_ms": llm_response.latency_ms,
                    }),
                    None,
                );
                logger.finalize();
            }

            return Ok(response_text);
        }

        // 启用工具调用模式
        let tool_schemas = self.build_tool_schemas();

        let mut current_iteration = 0;
        let mut final_response = String::new();

        while current_iteration < self.max_tool_iterations {
            current_iteration += 1;

            // 调用 LLM（Function Calling）
            let response = match self.base.llm.invoke_with_tools(
                messages.clone(),
                tool_schemas.clone(),
                Some("auto".to_string()),
            ) {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("❌ LLM 调用失败: {}", e);
                    if let Some(ref mut logger) = trace_logger {
                        logger.log_event(
                            "error",
                            serde_json::json!({"error_type": "LLM_ERROR", "message": e.to_string()}),
                            Some(current_iteration),
                        );
                    }
                    break;
                }
            };

            // 记录模型输出
            if let Some(ref mut logger) = trace_logger {
                let usage = &response.usage;
                logger.log_event(
                    "model_output",
                    serde_json::json!({
                        "content": response.content,
                        "tool_calls": response.tool_calls.len(),
                        "usage": {
                            "prompt_tokens": usage.prompt_tokens,
                            "completion_tokens": usage.completion_tokens,
                            "total_tokens": usage.total_tokens,
                        },
                    }),
                    Some(current_iteration),
                );
            }

            // 处理工具调用
            let tool_calls = response.tool_calls.clone();
            if tool_calls.is_empty() {
                // 没有工具调用，直接返回文本响应
                final_response = response.content.unwrap_or_else(|| "抱歉，我无法回答这个问题。".to_string());
                break;
            }

            // 将助手消息添加到历史
            let assistant_msg = Message {
                role: MessageRole::Assistant,
                content: response.content.clone().map(MessageContent::Text),
                tool_calls: Some(
                    tool_calls
                        .iter()
                        .map(|tc| crate::core::llm_resp_req::ToolCall {
                            id: tc.id.clone(),
                            call_type: Some("function".to_string()),
                            function: Some(crate::core::llm_resp_req::FunctionCall {
                                name: tc.function.as_ref().and_then(|f| f.name.clone()),
                                arguments: tc.function.as_ref().and_then(|f| f.arguments.clone()),
                            }),
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            };
            messages.push(assistant_msg);

            // 执行所有工具调用
            for tool_call in &tool_calls {
                let tool_name = tool_call.function.as_ref()
                    .and_then(|f| f.name.clone())
                    .unwrap_or_default();
                let tool_call_id = tool_call.id.clone().unwrap_or_default();

                // 解析参数（带正确的 continue 跳过）
                let args_str = tool_call.function.as_ref()
                    .and_then(|f| f.arguments.clone())
                    .unwrap_or_default();
                let arguments = match serde_json::from_str::<Value>(&args_str) {
                    Ok(args) => args,
                    Err(e) => {
                        eprintln!("❌ 工具参数解析失败: {}", e);
                        messages.push(Message {
                            role: MessageRole::Tool,
                            tool_call_id: Some(tool_call_id.clone()),
                            content: Some(MessageContent::Text(
                                format!("错误：参数格式不正确 - {}", e)
                            )),
                            ..Default::default()
                        });
                        continue;
                    }
                };

                // 记录工具调用（TraceLogger）
                if let Some(ref mut logger) = trace_logger {
                    logger.log_event(
                        "tool_call",
                        serde_json::json!({
                "tool_name": &tool_name,
                "tool_call_id": &tool_call_id,
                "args": &arguments,
            }),
                        Some(current_iteration),
                    );
                }

                // 执行工具
                let result = self.execute_tool_call(&tool_name, arguments);

                // 记录工具结果
                if let Some(ref mut logger) = trace_logger {
                    logger.log_event(
                        "tool_result",
                        serde_json::json!({
                "tool_name": &tool_name,
                "tool_call_id": &tool_call_id,
                "result": &result,
            }),
                        Some(current_iteration),
                    );
                }

                // 添加工具结果消息
                messages.push(Message {
                    role: MessageRole::Tool,
                    tool_call_id: Some(tool_call_id),
                    content: Some(MessageContent::Text(result)),
                    ..Default::default()
                });
            }
        }

        // 如果超过最大迭代次数，获取最后一次回答
        if current_iteration >= self.max_tool_iterations && final_response.is_empty() {
            let llm_response = self.base.llm.invoke(messages)?;
            final_response = llm_response.content;
        }

        // 保存到历史记录
        self.base.history_manager.append(Message::new_text(input_text, MessageRole::User));
        self.base.history_manager.append(Message::new_text(&final_response, MessageRole::Assistant));

        if let Some(ref mut logger) = trace_logger {
            let duration = session_start.elapsed().as_secs_f64();
            logger.log_event(
                "session_end",
                serde_json::json!({
                    "duration": duration,
                    "total_steps": current_iteration,
                    "final_answer": final_response,
                    "status": "success",
                }),
                None,
            );
            logger.finalize();
        }

        Ok(final_response)
    }

    fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>> {
        // 直接使用基类的工具 schema 构建逻辑，但注意基类中可能已有默认实现，
        // 这里调用 base 的 build_tool_schemas 或自己实现（这里复用 Agent trait 中的默认实现）
        // 因为 SimpleAgent 没有重写该方法，所以直接调用基类的。
        // 但我们在此需要自己实现，因为 AgentBase 没有直接提供该方法。
        if let Some(ref reg) = self.base.tool_registry {
            let reg = reg.lock().unwrap();
            let tools = reg.get_all_tools();
            tools.iter().map(|tool| {
                let mut properties = serde_json::Map::new();
                let mut required = Vec::new();
                for param in tool.get_parameters() {
                    let mut prop = serde_json::Map::new();
                    prop.insert("type".into(), Value::String(param.param_type));
                    prop.insert("description".into(), Value::String(param.description));
                    if let Some(default) = param.default {
                        prop.insert("default".into(), default);
                    }
                    let param_name = param.name.clone();
                    properties.insert(param_name.to_owned(), Value::Object(prop));
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
                    params.insert("required".into(), Value::Array(required.into_iter().map(|item|serde_json::json!(item)).collect()));
                }
                func.insert("parameters".into(), Value::Object(params));
                let mut schema = serde_json::Map::new();
                schema.insert("type".into(), Value::String("function".into()));
                schema.insert("function".into(), Value::Object(func));
                schema.into_iter().collect()
            }).collect()
        } else {
            Vec::new()
        }
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
        // 简单实现，完整上下文隔离可按需完善
        let start = Instant::now();
        let result = self.run(task, HashMap::new())?;
        let duration = start.elapsed().as_secs_f64();
        Ok(SubagentResult {
            success: true,
            summary: result.clone(),
            metadata: SubagentMetadata {
                steps: 1,
                tokens: 0,
                duration_seconds: duration,
                tools_used: vec![],
                error: None,
            },
        })
    }
}

impl SimpleAgent {
    /// 流式运行 Agent
    pub fn stream_run(
        &mut self,
        input_text: &str,
        _kwargs: Option<HashMap<String, String>>,
    ) -> impl Iterator<Item = Result<String, HelloAgentException>> + '_ {
        let messages = self.build_messages(input_text);
        let chunks = self.base.llm.stream_invoke(messages).unwrap_or_else(|_| Box::new(std::iter::empty()));
        let full_response = String::new();

        // 将流转换为迭代器，并在结束时保存历史
        let mut full = String::new();
        chunks
            .filter_map(|r| r.ok())
            .map(move |chunk| {
                full.push_str(&chunk);
                Ok(chunk)
            })
            // 注意：无法在迭代器结束时保存历史，因为迭代器是惰性的，需要外部处理。
            // 为简化，这里仅返回流。实际使用中，可以在外部收集完所有 chunk 后调用 add_message。
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// 异步流式执行
    pub async fn arun_stream(
        &mut self,
        input_text: &str,
        on_start: Option<LifecycleHook>,
        on_finish: Option<LifecycleHook>,
        on_error: Option<LifecycleHook>,
        kwargs: Option<HashMap<String, String>>,
    ) -> impl Stream<Item = AgentEvent> {
        // 发送开始事件
        let start_event = AgentEvent::new(
            EventType::AgentStart,
            self.name().to_string(),
            {
                let mut data = HashMap::new();
                data.insert("input_text".into(), Value::String(input_text.to_string()));
                data
            },
        );
        if let Some(ref hook) = on_start {
            hook(start_event.clone()).await;
        }

        // 构建消息
        let messages = self.build_messages(input_text);

        // 获取异步流
        let mut stream = self.base.llm.astream_invoke(messages).await;
        let mut full_response = String::new();
        let name = self.name().to_string();

        // 将 stream 转换为 AgentEvent 流
        let event_stream = async_stream::stream! {
            yield start_event;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        full_response.push_str(&chunk);
                        yield AgentEvent::new(
                            EventType::LlmChunk,
                            name.clone(),
                            {
                                let mut data = HashMap::new();
                                data.insert("chunk".into(), Value::String(chunk));
                                data
                            },
                        );
                    }
                    Err(e) => {
                        if let Some(ref hook) = on_error {
                            hook(AgentEvent::new(
                                EventType::AgentError,
                                name.clone(),
                                {
                                    let mut data = HashMap::new();
                                    data.insert("error".into(), Value::String(e.to_string()));
                                    data.insert("error_type".into(), Value::String("HelloAgentException".into()));
                                    data
                                },
                            )).await;
                        }
                        return;
                    }
                }
            }

            // 发送完成事件
            let finish_event = AgentEvent::new(
                EventType::AgentFinish,
                name.clone(),
                {
                    let mut data = HashMap::new();
                    data.insert("result".into(), Value::String(full_response.clone()));
                    data
                },
            );
            if let Some(ref hook) = on_finish {
                hook(finish_event.clone()).await;
            }
            yield finish_event;
        };

        event_stream
    }
}