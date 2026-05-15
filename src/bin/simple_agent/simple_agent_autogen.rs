use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use crate::simple_agent::simple_agent_client::{HelloAgentsLLM, Message};

// ==================== 智能体定义 ====================

/// 代理类型：标准 LLM 代理 或 用户代理（自动响应）
enum AgentKind {
    LLM,
    UserProxy { manual_response: String },
}

pub struct Agent {
    name: String,
    system_message: String,
    model_client: Option<Arc<HelloAgentsLLM>>, // 仅 LLM 代理需要
    kind: AgentKind,
}

impl Agent {
    pub fn new_llm(name: String, system_message: String, model_client: Arc<HelloAgentsLLM>) -> Self {
        Self {
            name,
            system_message,
            model_client: Some(model_client),
            kind: AgentKind::LLM,
        }
    }

    fn new_user_proxy(name: String, description: String, termination_message: String) -> Self {
        Self {
            name,
            system_message: description, // 用于控制台展示（原始 Python 中为 description）
            model_client: None,
            kind: AgentKind::UserProxy {
                manual_response: termination_message,
            },
        }
    }

    /// 根据对话历史生成下一条消息（仅 LLM 代理使用）
    pub async fn speak_llm(&self, history: &[Message]) -> Result<String> {
        let client = self
            .model_client
            .as_ref()
            .expect("LLM agent must have a model client");

        // 构建消息列表：系统提示 + 完整历史（无需额外提示）
        let mut messages = Vec::with_capacity(1 + history.len());
        messages.push(Message {
            role: "system".into(),
            content: self.system_message.clone(),
            name: None, // 系统消息通常不需要 name
        });
        messages.extend_from_slice(history);

        // 调用 LLM
        let response = client.think(messages, 0.7).await?;
        Ok(response)
    }

    /// 统一发言入口：根据代理类型决定是调用 LLM 还是直接返回固定文本
    pub async fn speak(&self, history: &[Message]) -> Result<(String, String)> {
        // 返回 (角色, 内容)
        match &self.kind {
            AgentKind::LLM => {
                let content = self.speak_llm(history).await?;
                Ok(("assistant".into(), content))
            }
            AgentKind::UserProxy { manual_response } => {
                // 用户代理不调用 LLM，直接返回终止消息
                // 注意：原始 UserProxyAgent 会检查上下文，这里简化为直接返回 TERMINATE
                Ok(("user".into(), manual_response.clone()))
            }
        }
    }
}

// ==================== 团队聊天 ====================
pub struct RoundRobinGroupChat {
    agents: Vec<Agent>,
    termination_text: String,
    max_turns: usize,
    history: Vec<Message>,
}

impl RoundRobinGroupChat {
    pub fn new(agents: Vec<Agent>, termination_text: String, max_turns: usize) -> Self {
        Self {
            agents,
            termination_text,
            max_turns,
            history: Vec::new(),
        }
    }

    pub async fn run(&mut self, task: &str) -> Result<()> {
        // 初始化历史：用户任务（角色 user，无 name，表示用户输入）
        self.history.push(Message {
            role: "user".into(),
            content: task.to_string(),
            name: None,
        });

        for turn in 0..self.max_turns {
            let agent = &self.agents[turn % self.agents.len()];

            println!("\n--- {} 的回合 ---", agent.name);

            // 调用代理发言，获取 (role, content)
            let (role, content) = agent.speak(&self.history).await?;

            // 显示发言内容（保留名称前缀仅用于控制台展示）
            println!("[{}] {}: {}", role, agent.name, content);

            // 将发言追加到历史，结构完全遵循 AutoGen
            self.history.push(Message {
                role,
                content: content.clone(),
                name: Some(agent.name.clone()),
            });

            // 检查终止条件
            if content.contains(&self.termination_text) {
                println!("✅ 检测到终止词，团队协作结束。");
                break;
            }
        }

        println!("\n协作完成。总消息数: {}", self.history.len());
        Ok(())
    }
}

// ==================== 工厂函数 ====================

pub fn create_product_manager(client: Arc<HelloAgentsLLM>) -> Agent {
    let system_message = r#"你是一位经验丰富的产品经理，专门负责软件产品的需求分析和项目规划。

你的核心职责包括：
1. **需求分析**：深入理解用户需求，识别核心功能和边界条件
2. **技术规划**：基于需求制定清晰的技术实现路径
3. **风险评估**：识别潜在的技术风险和用户体验问题
4. **协调沟通**：与工程师和其他团队成员进行有效沟通

当接到开发任务时，请按以下结构进行分析：
1. 需求理解与分析
2. 功能模块划分
3. 技术选型建议
4. 实现优先级排序
5. 验收标准定义

请简洁明了地回应，并在分析完成后说"请工程师开始实现"。"#;
    Agent::new_llm("ProductManager".into(), system_message.into(), client)
}

pub fn create_engineer(client: Arc<HelloAgentsLLM>) -> Agent {
    let system_message = r#"你是一位资深的软件工程师，擅长 Python 开发和 Web 应用构建。

你的技术专长包括：
1. **Python 编程**：熟练掌握 Python 语法和最佳实践
2. **Web 开发**：精通 Streamlit、Flask、Django 等框架
3. **API 集成**：有丰富的第三方 API 集成经验
4. **数据源**: 要求数据源从Tsanghi（沧海数据）的加密货币实时日线接口https://www.tsanghi.com/api/fin/crypto/daily/realtime?token=demo&ticker=BTC/USD&exchange_code=Binance
5. **错误处理**：注重代码的健壮性和异常处理

当收到开发任务时，请：
1. 仔细分析技术需求
2. 选择合适的技术方案
3. 编写完整的代码实现
4. 添加必要的注释和说明
5. 考虑边界情况和异常处理

请提供完整的可运行代码，并在完成后说"请代码审查员检查"。"#;
    Agent::new_llm("Engineer".into(), system_message.into(), client)
}

pub fn create_code_reviewer(client: Arc<HelloAgentsLLM>) -> Agent {
    let system_message = r#"你是一位经验丰富的代码审查专家，专注于代码质量和最佳实践。

你的审查重点包括：
1. **代码质量**：检查代码的可读性、可维护性和性能
2. **安全性**：识别潜在的安全漏洞和风险点
3. **最佳实践**：确保代码遵循行业标准和最佳实践
4. **错误处理**：验证异常处理的完整性和合理性

审查流程：
1. 仔细阅读和理解代码逻辑
2. 检查代码规范和最佳实践
3. 识别潜在问题和改进点
4. 提供具体的修改建议
5. 评估代码的整体质量

请提供具体的审查意见，完成后说"代码审查完成，请用户代理测试"。"#;
    Agent::new_llm("CodeReviewer".into(), system_message.into(), client)
}

pub fn create_user_proxy() -> Agent {
    let description = r#"用户代理，负责以下职责：
1. 代表用户提出开发需求
2. 执行最终的代码实现
3. 验证功能是否符合预期
4. 提供用户反馈和建议

完成测试后请回复 TERMINATE。"#;
    // 该代理不调用 LLM，直接返回终止词
    Agent::new_user_proxy("UserProxy".into(), description.into(), "TERMINATE".into())
}

