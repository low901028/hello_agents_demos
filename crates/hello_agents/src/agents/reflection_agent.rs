// reflection_agent.rs
// Reflection Agent 实现 - 自我反思与迭代优化的智能体

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use serde_json::Value;

use crate::core::llm_agent::AgentBase;
use crate::core::agent_trait::{Agent, SubagentMetadata, SubagentResult};
use crate::core::config::Config;
use crate::core::exceptions::HelloAgentException;
use crate::core::hello_agents_llm::HelloAgentsLLM;
use crate::core::llm_resp_req::{Message, MessageContent, MessageRole};
use crate::tools::tool_base::Tool;
use crate::tools::tool_filter::ToolFilter;
use crate::tools::tool_registry::ToolRegistry;
use crate::tools::tool_response::ToolStatus;

/// 简单的短期记忆模块，用于存储智能体的行动与反思轨迹。
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
        Self { records: Vec::new() }
    }

    /// 向记忆中添加一条新记录
    pub fn add_record(&mut self, record_type: &str, content: &str) {
        self.records.push(MemoryRecord {
            record_type: record_type.to_string(),
            content: content.to_string(),
        });
        println!("📝 记忆已更新，新增一条 '{}' 记录。", record_type);
    }

    /// 将所有记忆记录格式化为一个连贯的字符串文本
    pub fn get_trajectory(&self) -> String {
        let mut trajectory = String::new();
        for record in &self.records {
            match record.record_type.as_str() {
                "execution" => {
                    trajectory.push_str(&format!("--- 上一轮尝试 (代码) ---\n{}\n\n", record.content));
                }
                "reflection" => {
                    trajectory.push_str(&format!("--- 评审员反馈 ---\n{}\n\n", record.content));
                }
                _ => {}
            }
        }
        trajectory.trim().to_string()
    }

    /// 获取最近一次的执行结果
    pub fn get_last_execution(&self) -> String {
        for record in self.records.iter().rev() {
            if record.record_type == "execution" {
                return record.content.clone();
            }
        }
        String::new()
    }
}

/// Reflection Agent - 自我反思与迭代优化的智能体
///
/// 这个 Agent 能够：
/// 1. 执行初始任务
/// 2. 对结果进行自我反思
/// 3. 根据反思结果进行优化
/// 4. 迭代改进直到满意
/// 5. 支持工具调用（可选）
///
/// 特别适合代码生成、文档写作、分析报告等需要迭代优化的任务。
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
        llm: HelloAgentsLLM,
        system_prompt: Option<String>,
        config: Option<Config>,
        max_iterations: usize,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        enable_tool_calling: bool,
        max_tool_iterations: usize,
    ) -> Self {
        let default_system_prompt = "你是一个具有自我反思能力的AI助手。你的工作流程是：\n\
            1. 首先尝试完成用户的任务\n\
            2. 然后反思你的回答，找出可能的问题或改进空间\n\
            3. 根据反思结果优化你的回答\n\
            4. 如果回答已经很好，在反思时回复\"无需改进\"\n\n\
            请始终保持批判性思维，追求更高质量的输出。".to_string();

        let system_prompt = system_prompt.unwrap_or(default_system_prompt);
        let enable = enable_tool_calling && tool_registry.is_some();
        let working_dir = std::env::current_dir().unwrap_or_default();

        Self {
            base: AgentBase::new(name, llm, Some(system_prompt), config, tool_registry, Some(working_dir)),
            max_iterations,
            memory: Memory::new(),
            enable_tool_calling: enable,
            max_tool_iterations,
        }
    }

    /// 调用 LLM 并获取完整响应（支持 Function Calling）
    fn get_llm_response(&self, messages: Vec<Message>, kwargs: Option<HashMap<String, String>>) -> Result<String, HelloAgentException> {
        // 如果没有启用工具调用，直接返回
        if !self.enable_tool_calling || self.base.tool_registry.is_none() {
            //let mut final_kwargs = HashMap::new();
            // 合并 kwargs，简化处理
            let llm_response = self.base.llm.invoke(messages)?;
            return Ok(llm_response.content);
        }

        // 启用工具调用模式
        let tool_schemas = self.build_tool_schemas();
        let mut current_messages = messages;
        let mut iteration = 0;

        while iteration < self.max_tool_iterations {
            iteration += 1;

            let response = match self.base.llm.invoke_with_tools(
                current_messages.clone(),
                tool_schemas.clone(),
                Some("auto".to_string()),
            ) {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("❌ LLM 调用失败: {}", e);
                    break;
                }
            };

            let tool_calls = response.tool_calls.clone();
            if tool_calls.is_empty() {
                return Ok(response.content.unwrap_or_default());
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
            current_messages.push(assistant_msg);

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
                        current_messages.push(Message {
                            role: MessageRole::Tool,
                            tool_call_id: Some(tool_call_id.clone()),
                            content: Some(MessageContent::Text(format!("错误：参数格式不正确 - {}", e))),
                            ..Default::default()
                        });
                        continue;
                    }
                };

                // 执行工具
                let result = self.execute_tool_call(&tool_name, arguments);

                // 添加工具结果消息
                current_messages.push(Message {
                    role: MessageRole::Tool,
                    tool_call_id: Some(tool_call_id),
                    content: Some(MessageContent::Text(result)),
                    ..Default::default()
                });
            }
        }

        // 如果超过最大迭代次数，获取最后一次回答
        let llm_response = self.base.llm.invoke(current_messages)?;
        Ok(llm_response.content)
    }

    /// 执行初始任务
    fn execute_task(&self, task: &str, kwargs: Option<HashMap<String, String>>) -> Result<String, HelloAgentException> {
        let messages = vec![
            Message::new_text(self.base.system_prompt.as_ref().unwrap_or(&String::new()), MessageRole::System),
            Message::new_text(&format!("请完成以下任务：\n\n{}", task), MessageRole::User),
        ];
        self.get_llm_response(messages, kwargs)
    }

    /// 对结果进行反思
    fn reflect_on_result(&self, task: &str, result: &str, kwargs: Option<HashMap<String, String>>) -> Result<String, HelloAgentException> {
        let messages = vec![
            Message::new_text(self.base.system_prompt.as_ref().unwrap_or(&String::new()), MessageRole::System),
            Message::new_text(&format!(
                "请仔细审查以下回答，并找出可能的问题或改进空间：\n\n# 原始任务:\n{}\n\n# 当前回答:\n{}\n\n请分析这个回答的质量，指出不足之处，并提出具体的改进建议。\n如果回答已经很好，请回答\"无需改进\"。",
                task, result
            ), MessageRole::User),
        ];
        self.get_llm_response(messages, kwargs)
    }

    /// 根据反馈优化结果
    fn refine_result(&self, task: &str, last_attempt: &str, feedback: &str, kwargs: Option<HashMap<String, String>>) -> Result<String, HelloAgentException> {
        let messages = vec![
            Message::new_text(self.base.system_prompt.as_ref().unwrap_or(&String::new()), MessageRole::System),
            Message::new_text(&format!(
                "请根据反馈意见改进你的回答：\n\n# 原始任务:\n{}\n\n# 上一轮回答:\n{}\n\n# 反馈意见:\n{}\n\n请提供一个改进后的回答。",
                task, last_attempt, feedback
            ), MessageRole::User),
        ];
        self.get_llm_response(messages, kwargs)
    }
}

#[async_trait]
impl Agent for ReflectionAgent {
    fn name(&self) -> &str {
        &self.base.name
    }

    fn run(&mut self, input_text: &str, kwargs: HashMap<String, String>) -> Result<String, HelloAgentException> {
        println!("\n🤖 {} 开始处理任务: {}", self.base.name, input_text);

        // 重置记忆
        self.memory = Memory::new();

        // 1. 初始执行
        println!("\n--- 正在进行初始尝试 ---");
        let initial_result = self.execute_task(input_text, None)?;
        self.memory.add_record("execution", &initial_result);

        // 2. 迭代循环：反思与优化
        for i in 0..self.max_iterations {
            println!("\n--- 第 {}/{} 轮迭代 ---", i + 1, self.max_iterations);

            // a. 反思
            println!("\n-> 正在进行反思...");
            let last_result = self.memory.get_last_execution();
            let feedback = self.reflect_on_result(input_text, &last_result, None)?;
            self.memory.add_record("reflection", &feedback);

            // b. 检查是否需要停止
            if feedback.contains("无需改进") || feedback.to_lowercase().contains("no need for improvement") {
                println!("\n✅ 反思认为结果已无需改进，任务完成。");
                break;
            }

            // c. 优化
            println!("\n-> 正在进行优化...");
            let refined_result = self.refine_result(input_text, &last_result, &feedback, None)?;
            self.memory.add_record("execution", &refined_result);
        }

        let final_result = self.memory.get_last_execution();
        println!("\n--- 任务完成 ---\n最终结果:\n{}", final_result);

        // 保存到历史记录
        self.base.history_manager.append(Message::new_text(input_text, MessageRole::User));
        self.base.history_manager.append(Message::new_text(&final_result, MessageRole::Assistant));

        Ok(final_result)
    }

    fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>> {
        // 复用基类的工具 schema 构建逻辑
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
                    let param_name = param.name;
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
                    params.insert("required".into(), Value::Array(required.into_iter().map(Value::String).collect()));
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
        // 简单实现：直接调用 run
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

impl ReflectionAgent {
    /// 添加工具到 Agent
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

    // 流式接口未在此完整实现，但可根据需要补充
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::exceptions::HelloAgentException;

    // 由于需要真实 LLM，这里仅测试 Memory 和基本逻辑
    #[test]
    fn test_memory() {
        let mut mem = Memory::new();
        mem.add_record("execution", "尝试1");
        mem.add_record("reflection", "反思1");
        assert_eq!(mem.get_last_execution(), "尝试1");
        let trajectory = mem.get_trajectory();
        assert!(trajectory.contains("尝试1"));
        assert!(trajectory.contains("反思1"));
    }
}