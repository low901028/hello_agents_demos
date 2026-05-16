/// 角色扮演中的消息封装
/// 对应 CAMEL 的 ChatMessage
#[derive(Debug, Clone)]
pub struct RoleMessage {
    pub role_name: String,
    pub content: String,
}

impl RoleMessage {
    pub fn new(role_name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role_name: role_name.into(),
            content: content.into(),
        }
    }
}

/// 单步结果：助手回复 + 用户回复
pub struct StepResult {
    pub assistant: RoleMessage,
    pub user: RoleMessage,
}

/// 模型配置
pub struct ModelConfig {
    pub model: String,
    pub api_key: String,
    pub base_url: String,
}