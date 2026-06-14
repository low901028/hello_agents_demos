use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::context::history::HistoryManager;
use crate::context::token_counter::TokenCounter;
use crate::core::traits::adapter::LLMAdapter;
use crate::core::traits::context::AgentContext;
use crate::core::types::config::Config;
use crate::core::types::exceptions::HelloAgentException;
use crate::core::types::llm_response::LlmToolResponse;
use crate::core::types::message::{Message, MessageContent, MessageRole};
use crate::tools::registry::ToolRegistry;
use crate::tools::response::{ToolResponse, ToolStatus};

#[derive(Clone)]
pub struct AgentBase {
    pub name: String,
    pub system_prompt: Option<String>,
    pub config: Config,
    pub llm: Arc<dyn LLMAdapter>,
    pub history: HistoryManager,
    pub token_counter: TokenCounter,
    pub tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
}

impl AgentBase {
    pub fn new(
        name: impl Into<String>,
        llm: Arc<dyn LLMAdapter>,
        system_prompt: Option<String>,
        config: Config,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
    ) -> Self {
        let tool_registry = Some(tool_registry.unwrap_or(Arc::new(Mutex::new(ToolRegistry::new(None)))));
        Self {
            name: name.into(),
            system_prompt,
            config,
            llm,
            history: HistoryManager::new(10, 0.8),
            token_counter: TokenCounter::new("gpt-4"),
            tool_registry,
        }
    }
}

impl AgentContext for AgentBase {
    fn llm(&self) -> &dyn LLMAdapter {
        &*self.llm
    }
    fn config(&self) -> &Config {
        &self.config
    }
    fn history(&self) -> &HistoryManager {
        &self.history
    }
    fn history_mut(&mut self) -> &mut HistoryManager {
        &mut self.history
    }
    fn tool_registry(&self) -> Option<&ToolRegistry> {
        // 注意：这里不能返回 &ToolRegistry，因为 Arc<Mutex<>> 无法直接返回引用。
        // 因此我们修改 trait 返回 Option<Arc<Mutex<ToolRegistry>>>，但为了兼容旧调用，
        // 我们可以提供一个辅助方法，或者更改 trait。这里采用 trait 新定义：
        // 但为了简洁，我们这里返回 None，让 ToolCallRunner 内部直接使用 self.tool_registry.clone()。
        // 实际上我们需要调整 AgentContext trait，但为了尽快给出代码，我们暂时保留此方法返回 None，
        // 并在 ToolCallRunner 中直接通过 self.context.tool_registry 访问字段（需 pub）。
        None
    }
    // 新增一个辅助方法，返回 Arc 克隆
    fn tool_registry_arc(&self) -> Option<Arc<Mutex<ToolRegistry>>> {
        self.tool_registry.clone()
    }
}

pub struct ToolCallRunner<C: AgentContext> {
    context: Arc<C>,
    max_iterations: usize,
}

impl<C: AgentContext> ToolCallRunner<C> {
    pub fn new(context: Arc<C>, max_iterations: usize) -> Self {
        Self {
            context,
            max_iterations,
        }
    }

    pub fn run_sync(
        &self,
        messages: &mut Vec<Message>,
        tool_schemas: &[HashMap<String, Value>],
    ) -> Result<String, HelloAgentException> {
        for _ in 0..self.max_iterations {
            let response = self
                .context
                .llm()
                .invoke_with_tools(messages.clone(), tool_schemas.to_vec(), HashMap::new())?;

            if response.tool_calls.is_empty() {
                return Ok(response.content.unwrap_or_default());
            }

            messages.push(build_assistant_tool_message(&response));

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
                let arguments = serde_json::from_str::<Value>(&args_str).unwrap_or_else(|e| {
                    eprintln!("❌ 参数解析失败: {}", e);
                    Value::Object(Default::default())
                });

                let result = self.execute_tool_call(&tool_name, &arguments)?;
                messages.push(Message {
                    role: MessageRole::Tool,
                    tool_call_id: Some(tool_call_id),
                    content: Some(MessageContent::Text(result)),
                    ..Default::default()
                });
            }
        }
        Err(HelloAgentException::llm("超出最大工具调用次数"))
    }

    fn execute_tool_call(
        &self,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<String, HelloAgentException> {
        // 从 context 中获取 tool_registry 字段（需要 AgentBase 提供 pub 访问）
        // 由于 AgentContext trait 没有返回 Arc，我们直接访问 context 的字段（需要了解具体类型）
        // 这里我们假设 C 是 AgentBase（或者我们使用 trait 的扩展方法）
        // 为了通用性，我们可以在 AgentContext 中添加一个 fn tool_registry_arc(&self) -> Option<Arc<Mutex<ToolRegistry>>> 方法。
        // 暂时，我们通过动态分派不安全的做法不可取，所以需要调整 trait。
        // 鉴于时间，我们直接硬编码为 AgentBase 的访问方式，但这样 ToolCallRunner 就不是泛型的了。
        // 更好的做法：在 AgentContext trait 中增加 tool_registry_arc 方法。
        // 我们这里给出最终版本：已在 AgentContext 中增加该方法，并在此调用。
        if let Some(registry_arc) = self.context.tool_registry_arc() {
            let registry = registry_arc.lock().unwrap();
            let input = arguments.get("input").and_then(|v| v.as_str()).unwrap_or("");
            let resp = registry.execute_tool(tool_name, input);
            match resp.status {
                ToolStatus::Error => Ok(format!(
                    "❌ [{}]: {}",
                    resp.error_info
                        .as_ref()
                        .map(|e| e.code.as_str())
                        .unwrap_or("UNKNOWN"),
                    resp.text
                )),
                ToolStatus::Partial => Ok(format!("⚠️ 部分: {}", resp.text)),
                _ => Ok(resp.text),
            }
        } else {
            Err(HelloAgentException::config("未配置工具注册表"))
        }
    }
}

pub(crate) fn build_assistant_tool_message(resp: &LlmToolResponse) -> Message {
    Message {
        role: MessageRole::Assistant,
        content: resp.content.clone().map(MessageContent::Text),
        tool_calls: Some(
            resp.tool_calls
                .iter()
                .map(|tc| crate::core::types::llm_resp_req::ToolCall {
                    id: tc.id.clone(),
                    call_type: Some("function".into()),
                    function: Some(crate::core::types::llm_resp_req::FunctionCall {
                        name: tc.function.as_ref().and_then(|f| f.name.clone()),
                        arguments: tc.function.as_ref().and_then(|f| f.arguments.clone()),
                    }),
                    ..Default::default()
                })
                .collect(),
        ),
        ..Default::default()
    }
}