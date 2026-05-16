use anyhow::Result;
use std::sync::Arc;

use crate::simple_agent::simple_agent_client::{HelloAgentsLLM, Message};
use super::role_types::{ModelConfig, RoleMessage, StepResult};

/// 系统提示模板（使用 C 的原始字符串避免转义问题）
const ROLE_PLAYING_SYSTEM_PROMPT: &str = r#"永远不要忘记你是一个{role}，我是一个{user_role}，你永远不要变成除{role}之外的任何角色。
你的任务是在不切换角色的情况下与我协作完成任务。
任务开始前，请充分理解任务，并仔细思考后再给出回应。
永远不要给出沉浸式对话或非正式的聊天。
你的回应必须严格以{role}的身份进行。
任务描述：{task}

当任务协作完成时，必须在回复的最后一行明确输出：CAMEL_TASK_DONE
"#;

const ASSISTANT_INCEPTION_PROMPT: &str = r#"永远不要忘记你是一个{role}，我是一个{user_role}，你永远不要变成除{role}之外的任何角色。
你的回复应该全面且具有建设性，推动任务向前发展。
首先，请分析任务需求并制定一个清晰的协作计划。
你的回应必须严格以{role}的身份进行。
"#;

const USER_INCEPTION_PROMPT: &str = r#"永远不要忘记你是一个{user_role}，我是一个{role}，你永远不要变成除{user_role}之外的任何角色。
你的任务是与{role}协作完成创作任务。
请根据{role}的指导和反馈进行创作，并主动提出改进建议。
当最终作品完成且满足要求时，必须在回复中包含"CAMEL_TASK_DONE"。
你的回应必须严格以{user_role}的身份进行。
"#;

/// CAMEL RolePlaying 的核心实现
pub struct RolePlaying {
    assistant_role: String,
    user_role: String,
    task_prompt: String,
    client: Arc<HelloAgentsLLM>,
    /// 对话历史（用于上下文）
    assistant_history: Vec<Message>,
    user_history: Vec<Message>,
    /// 扩展的系统提示（初始化后生成）
    extended_task_prompt: String,
}

impl RolePlaying {
    /// 创建角色扮演会话
    /// - `assistant_role`: 助手角色名（如"心理学家"）
    /// - `user_role`: 用户角色名（如"作家"）
    /// - `task_prompt`: 协作任务描述
    /// - `client`: LLM 客户端
    pub fn new(
        assistant_role: impl Into<String>,
        user_role: impl Into<String>,
        task_prompt: impl Into<String>,
        client: Arc<HelloAgentsLLM>,
    ) -> Self {
        let assistant_role = assistant_role.into();
        let user_role = user_role.into();
        let task_prompt = task_prompt.into();

        // 生成扩展任务描述（等价 Python 的 role_play_session.task_prompt）
        let extended_task_prompt = ROLE_PLAYING_SYSTEM_PROMPT
            .replace("{role}", &assistant_role)
            .replace("{user_role}", &user_role)
            .replace("{task}", &task_prompt);

        Self {
            assistant_role,
            user_role,
            task_prompt,
            client,
            assistant_history: Vec::new(),
            user_history: Vec::new(),
            extended_task_prompt,
        }
    }

    /// 获取扩展任务描述（等价 Python 的 role_play_session.task_prompt）
    pub fn task_prompt(&self) -> &str {
        &self.extended_task_prompt
    }

    /// 初始化对话，返回第一条用户消息
    /// 等价 Python 的 role_play_session.init_chat()
    pub async fn init_chat(&mut self) -> Result<RoleMessage> {
        // 构建助手初始提示
        let assistant_inception = ASSISTANT_INCEPTION_PROMPT
            .replace("{role}", &self.assistant_role)
            .replace("{user_role}", &self.user_role);

        // 助手先发言（制定计划）
        let messages = vec![
            Message {
                role: "system".into(),
                content: self.extended_task_prompt.clone(),
                name: Some("系统".into()),
            },
            Message {
                role: "user".into(),
                content: assistant_inception,
                name: Some("系统".into()),
            },
        ];

        let assistant_response = self.client.think(messages, 0.7,None).await?;

        // 构建用户初始提示
        let user_inception = USER_INCEPTION_PROMPT
            .replace("{user_role}", &self.user_role)
            .replace("{role}", &self.assistant_role);

        // 用户根据助手的计划开始回应
        let user_messages = vec![
            Message {
                role: "system".into(),
                content: self.extended_task_prompt.clone(),
                name: Some("系统".into()),
            },
            Message {
                role: "user".into(),
                content: user_inception,
                name: Some("系统".into()),
            },
            Message {
                role: "assistant".into(),
                content: assistant_response.clone(),
                name: Some(self.assistant_role.clone()),
            },
        ];

        let user_response = self.client.think(user_messages, 0.7, None).await?;

        // 记录历史
        self.assistant_history.push(Message {
            role: "assistant".into(),
            content: assistant_response.clone(),
            name: Some(self.assistant_role.clone()),
        });
        self.user_history.push(Message {
            role: "assistant".into(),
            content: user_response.clone(),
            name: Some(self.user_role.clone()),
        });

        Ok(RoleMessage::new(&self.user_role, user_response))
    }

    /// 执行一步对话：用户消息 → 助手回复 + 用户回复
    /// 等价 Python 的 role_play_session.step(input_msg)
    pub async fn step(&mut self, input_msg: &RoleMessage) -> Result<StepResult> {
        // 1. 构建助手上下文
        let mut assistant_messages = vec![
            Message {
                role: "system".into(),
                content: self.extended_task_prompt.clone(),
                name: Some("系统".into()),
            },
        ];
        // 交替添加历史对话
        for (user_msg, assistant_msg) in self.user_history.iter().zip(self.assistant_history.iter()) {
            assistant_messages.push(user_msg.clone());
            assistant_messages.push(assistant_msg.clone());
        }
        // 添加当前用户消息
        assistant_messages.push(Message {
            role: "user".into(),
            content: input_msg.content.clone(),
            name: Some(self.user_role.clone()),
        });

        let assistant_response = self.client.think(assistant_messages, 0.7, None).await?;

        // 2. 构建用户上下文
        let mut user_messages = vec![
            Message {
                role: "system".into(),
                content: self.extended_task_prompt.clone(),
                name: Some("系统".into()),
            },
        ];
        for (user_msg, assistant_msg) in self.user_history.iter().zip(self.assistant_history.iter()) {
            user_messages.push(user_msg.clone());
            user_messages.push(assistant_msg.clone());
        }
        user_messages.push(Message {
            role: "assistant".into(),
            content: assistant_response.clone(),
            name: Some(self.assistant_role.clone()),
        });

        let user_response = self.client.think(user_messages, 0.7, None).await?;

        // 3. 更新历史
        self.assistant_history.push(Message {
            role: "assistant".into(),
            content: assistant_response.clone(),
            name: Some(self.assistant_role.clone()),
        });
        self.user_history.push(Message {
            role: "assistant".into(),
            content: user_response.clone(),
            name: Some(self.user_role.clone()),
        });

        Ok(StepResult {
            assistant: RoleMessage::new(&self.assistant_role, assistant_response),
            user: RoleMessage::new(&self.user_role, user_response),
        })
    }
}