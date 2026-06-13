//! plan_solve_agent.rs
//! Plan and Solve Agent 实现 - 分解规划与逐步执行的智能体

use std::collections::HashMap;
use std::ops::Deref;
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
use crate::tools::tool_response::{ToolResponse, ToolStatus};

// ------------------------------------------------------------
// Planner - 负责将复杂问题分解为简单步骤（使用 Function Calling）
// ------------------------------------------------------------
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

    /// 生成执行计划（使用 Function Calling）
    pub fn plan(&self, question: &str, kwargs: Option<HashMap<String, String>>) -> Result<Vec<String>, HelloAgentException> {
        println!("--- 正在生成计划 ---");

        // 定义计划生成工具
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
            Message::new_text(&format!("请为以下问题生成详细的执行计划：\n\n{}", question), MessageRole::User),
        ];

        let tools = vec![{
            let mut m = HashMap::new();
            m.insert("type".to_string(), Value::String("function".to_string()));
            m.insert("function".to_string(), plan_tool["function"].clone());
            m
        }];

        // 强制调用 generate_plan
        let tool_choice = serde_json::json!({
            "type": "function",
            "function": {"name": "generate_plan"}
        });

        let response = self.llm.invoke_with_tools(
            messages,
            tools,
            Some(tool_choice.to_string()),
        )?;

        // 提取工具调用结果
        if let Some(ref tool_calls) = Some(response.tool_calls) {
            if let Some(first) = tool_calls.first() {
                let args_str = first.function.as_ref()
                    .and_then(|f| f.arguments.clone())
                    .unwrap_or_default();
                let arguments: Value = serde_json::from_str(&args_str).unwrap_or_default();
                let steps: Vec<String> = arguments["steps"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default();

                println!("✅ 计划已生成:");
                for (i, step) in steps.iter().enumerate() {
                    println!("  {}. {}", i + 1, step);
                }
                return Ok(steps);
            }
        }

        println!("❌ 模型未返回计划工具调用");
        Ok(Vec::new())
    }
}

// ------------------------------------------------------------
// Executor - 负责按计划逐步执行（支持 Function Calling）
// ------------------------------------------------------------
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

    /// 按计划执行任务（支持 Function Calling）
    pub fn execute(&self, question: &str, plan: &[String], kwargs: Option<HashMap<String, String>>) -> Result<String, HelloAgentException> {
        let mut history: Vec<HashMap<String, String>> = Vec::new();
        let mut final_answer = String::new();

        println!("\n--- 正在执行计划 ---");
        for (i, step) in plan.iter().enumerate() {
            let step_num = i + 1;
            println!("\n-> 正在执行步骤 {}/{}: {}", step_num, plan.len(), step);

            // 构建上下文消息
            let context = format!(
                "# 原始问题:\n{}\n\n# 完整计划:\n{}\n\n# 历史步骤与结果:\n{}\n\n# 当前步骤:\n{}\n\n请执行当前步骤并给出结果。",
                question,
                self.format_plan(plan),
                if history.is_empty() { "无".to_string() } else { self.format_history(&history) },
                step
            );

            // 执行单个步骤（支持工具调用）
            let response_text = self.execute_step(&context, kwargs.clone())?;

            let mut record = HashMap::new();
            record.insert("step".to_string(), step.clone());
            record.insert("result".to_string(), response_text.clone());
            history.push(record);
            final_answer = response_text.clone();
            println!("✅ 步骤 {} 已完成，结果: {}", step_num, final_answer);
        }

        Ok(final_answer)
    }

    fn format_plan(&self, plan: &[String]) -> String {
        plan.iter().enumerate()
            .map(|(i, step)| format!("{}. {}", i + 1, step))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_history(&self, history: &[HashMap<String, String>]) -> String {
        history.iter().enumerate()
            .map(|(i, h)| {
                let step = h.get("step").cloned().unwrap_or_default();
                let result = h.get("result").cloned().unwrap_or_default();
                format!("步骤 {}: {}\n结果: {}", i + 1, step, result)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// 执行单个步骤（支持 Function Calling）
    fn execute_step(&self, context: &str, kwargs: Option<HashMap<String, String>>) -> Result<String, HelloAgentException> {
        let mut messages = vec![
            Message::new_text(&self.system_prompt, MessageRole::System),
            Message::new_text(context, MessageRole::User),
        ];

        // 如果没有启用工具调用，直接返回
        if !self.enable_tool_calling || self.tool_registry.is_none() {
            let llm_response = self.llm.invoke(messages)?;
            return Ok(llm_response.content);
        }

        // 启用工具调用模式
        let tool_schemas = self.build_tool_schemas()?;
        let mut iteration = 0;

        while iteration < self.max_tool_iterations {
            iteration += 1;

            let response = match self.llm.invoke_with_tools(
                messages.clone(),
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

                // 执行工具
                let result = self.execute_tool_internal(&tool_name, &arguments)?;

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
        let llm_response = self.llm.invoke(messages)?;
        Ok(llm_response.content)
    }

    /// 构建工具 schemas（从注册表获取）
    fn build_tool_schemas(&self) -> Result<Vec<HashMap<String, Value>>, HelloAgentException> {
        if let Some(ref reg) = self.tool_registry {
            let reg = reg.lock().unwrap();
            let tools = reg.get_all_tools();
            let schemas = tools.iter().map(|tool| {
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
                schema.into_iter().collect()
            }).collect();
            Ok(schemas)
        } else {
            Ok(Vec::new())
        }
    }

    /// 执行单个工具调用
    fn execute_tool_internal(&self, tool_name: &str, arguments: &Value) -> Result<String, HelloAgentException> {
        if let Some(ref reg) = self.tool_registry {
            let mut reg = reg.lock().unwrap();
            let input_str = arguments.get("input").and_then(|v| v.as_str()).unwrap_or("");
            let resp = reg.execute_tool(tool_name, input_str);
            match resp.status {
                ToolStatus::Error => {
                    let code = resp.error_info.as_ref().map(|e| e.code.as_str()).unwrap_or("UNKNOWN");
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

// ------------------------------------------------------------
// PlanSolveAgent - 分解规划与逐步执行的智能体
// ------------------------------------------------------------
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
        config: Option<Config>,
        planner_prompt: Option<String>,
        executor_prompt: Option<String>,
        tool_registry: Option<Arc<Mutex<ToolRegistry>>>,
        enable_tool_calling: bool,
        max_tool_iterations: usize,
    ) -> Self {
        let working_dir = std::env::current_dir().unwrap_or_default();

        let planner = Planner::new(llm.clone(), planner_prompt);
        let executor = Executor::new(
            llm.clone(),
            executor_prompt,
            tool_registry.clone(),
            enable_tool_calling,
            max_tool_iterations,
        );
        Self {
            base: AgentBase::new(name, llm.clone(), system_prompt, config, tool_registry, Some(working_dir)),
            planner,
            executor,
        }
    }
}

#[async_trait]
impl Agent for PlanSolveAgent {
    fn name(&self) -> &str {
        &self.base.name
    }

    fn run(&mut self, input_text: &str, kwargs: HashMap<String, String>) -> Result<String, HelloAgentException> {
        println!("\n🤖 {} 开始处理问题: {}", self.base.name, input_text);

        // 1. 生成计划
        let plan = self.planner.plan(input_text, Some(kwargs.clone()))?;
        if plan.is_empty() {
            let final_answer = "无法生成有效的行动计划，任务终止。".to_string();
            println!("\n--- 任务终止 ---\n{}", final_answer);

            self.base.history_manager.append(Message::new_text(input_text, MessageRole::User));
            self.base.history_manager.append(Message::new_text(&final_answer, MessageRole::Assistant));
            return Ok(final_answer);
        }

        // 2. 执行计划
        let final_answer = self.executor.execute(input_text, &plan, Some(kwargs))?;
        println!("\n--- 任务完成 ---\n最终答案: {}", final_answer);

        self.base.history_manager.append(Message::new_text(input_text, MessageRole::User));
        self.base.history_manager.append(Message::new_text(&final_answer, MessageRole::Assistant));

        Ok(final_answer)
    }

    fn build_tool_schemas(&self) -> Vec<HashMap<String, Value>> {
        // PlanSolveAgent 不使用自己的工具 schemas，而是由 Executor 内部构建
        Vec::new()
    }

    fn execute_tool_call(&self, _tool_name: &str, _arguments: Value) -> String {
        // 由 Executor 负责执行
        "".to_string()
    }

    fn run_as_subagent(
        &mut self,
        task: &str,
        _tool_filter: Option<Box<dyn ToolFilter>>,
        return_summary: bool,
        _max_steps_override: Option<usize>,
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

// ------------------------------------------------------------
// 测试用例
// ------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_format() {
        // 无 LLM 的简单测试无法执行，仅测试构造
        // 实际测试需要 mock LLM
    }

    #[test]
    fn test_executor_format_functions() {
        // 直接测试格式化函数
        let llm = unimplemented!();
        let executor = Executor::new(llm, None, None, false, 3);
        let plan = vec!["步骤1".to_string(), "步骤2".to_string()];
        let formatted = executor.format_plan(&plan);
        assert!(formatted.contains("1. 步骤1"));
        assert!(formatted.contains("2. 步骤2"));

        let history = vec![
            {
                let mut m = HashMap::new();
                m.insert("step".to_string(), "步骤1".to_string());
                m.insert("result".to_string(), "结果1".to_string());
                m
            },
        ];
        let history_str = executor.format_history(&history);
        assert!(history_str.contains("步骤1"));
        assert!(history_str.contains("结果1"));
    }
}