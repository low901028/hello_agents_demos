use std::collections::HashMap;
use std::env::consts::ARCH;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;

use crate::agents::base::{AgentBase, ToolCallRunner};
use crate::core::traits::agent::{Agent, SubagentMetadata, SubagentResult};
use crate::core::traits::context::AgentContext;
use crate::core::traits::tool::Tool;
use crate::core::types::config::Config;
use crate::core::types::exceptions::HelloAgentException;
use crate::core::types::message::{Message, MessageRole};
use crate::tools::filter::ToolFilter;
use crate::tools::registry::ToolRegistry;
use crate::tools::response::ToolStatus;

pub struct SimpleAgent {
    base: AgentBase,
    enable_tool_calling: bool,
    runner: Option<ToolCallRunner<AgentBase>>,
}

impl SimpleAgent {
    pub fn new(
        name: &str,
        llm: Arc<dyn crate::core::traits::adapter::LLMAdapter>,
        system_prompt: Option<String>,
        config: Config,
        tool_registry: Option<Arc<Mutex<crate::tools::registry::ToolRegistry>>>,
        enable_tool_calling: bool,
        max_tool_iterations: usize,
    ) -> Self {
        let tool_registry = Some(tool_registry.unwrap_or(Arc::new(Mutex::new(ToolRegistry::new(None)))));
        let base = AgentBase::new(
            name,
            llm,
            system_prompt,
            config,
            tool_registry,
        );
        let runner = if enable_tool_calling {
            Some(ToolCallRunner::new(
                Arc::new(base.clone()),
                max_tool_iterations,
            ))
        } else {
            None
        };
        Self {
            base,
            enable_tool_calling,
            runner,
        }
    }

    // ----------------- 工具管理方法 -----------------
    /// 添加工具到 Agent，自动创建 ToolRegistry 并启用工具调用（如果需要）
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

    /// 移除指定名称的工具
    pub fn remove_tool(&mut self, tool_name: &str) -> bool {
        if let Some(ref reg) = self.base.tool_registry {
            let mut reg = reg.lock().unwrap();
            reg.unregister(tool_name);
            true
        } else {
            false
        }
    }

    /// 列出所有已注册工具的名称
    pub fn list_tools(&self) -> Vec<String> {
        if let Some(ref reg) = self.base.tool_registry {
            let reg = reg.lock().unwrap();
            reg.list_tools()
        } else {
            Vec::new()
        }
    }

    /// 检查是否启用了工具调用
    pub fn has_tools(&self) -> bool {
        self.enable_tool_calling && self.base.tool_registry.is_some()
    }

    fn build_messages(&self, input_text: &str) -> Vec<Message> {
        let mut messages = Vec::new();
        if let Some(ref sp) = self.base.system_prompt {
            messages.push(Message::new_text(sp, MessageRole::System));
        }
        messages.extend(self.base.history.get_history());
        messages.push(Message::new_text(input_text, MessageRole::User));
        messages
    }
}

#[async_trait]
impl Agent for SimpleAgent {
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
        let mut messages = self.build_messages(input_text);

        // 无工具调用模式
        if !self.enable_tool_calling || self.runner.is_none() {
            let resp = self.base.llm.invoke(messages, HashMap::new())?;
            self.base
                .history
                .append(Message::new_text(input_text, MessageRole::User));
            self.base
                .history
                .append(Message::new_text(&resp.content, MessageRole::Assistant));
            return Ok(resp.content);
        }

        // 工具调用模式
        let schemas = self.build_tool_schemas();
        let final_resp = self
            .runner
            .as_ref()
            .unwrap()
            .run_sync(&mut messages, &schemas)?;

        self.base
            .history
            .append(Message::new_text(input_text, MessageRole::User));
        self.base
            .history
            .append(Message::new_text(&final_resp, MessageRole::Assistant));
        Ok(final_resp)
    }
    
    fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>> {
        if let Some(reg_arc) = self.base.tool_registry_arc() {
            let reg = reg_arc.lock().unwrap();
            reg.get_all_tools()
                .iter()
                .map(|tool| {
                    let schema_val = tool.to_openai_schema();
                    if let Value::Object(map) = schema_val {
                        map.into_iter().collect()
                    } else {
                        HashMap::new()
                    }
                })
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

impl crate::tools::builtin::task::SubagentExecutor for SimpleAgent {
    fn run_as_subagent(
        &mut self,
        task: &str,
        tool_filter: Option<Box<dyn ToolFilter>>,
        return_summary: bool,
        max_steps_override: Option<usize>,
    ) -> Result<SubagentResult, HelloAgentException> {
        <Self as Agent>::run_as_subagent(
            self,
            task,
            tool_filter,
            return_summary,
            max_steps_override,
        )
    }
}
