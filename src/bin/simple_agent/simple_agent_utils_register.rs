use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
// ==================== 工具注册/执行器 ====================

/// 工具函数类型：异步，参数为字符串，返回字符串结果
#[async_trait]
pub trait ToolFunction: Send + Sync {
    async fn call(&self, input: &str) -> String;
}

/// 工具描述
struct ToolEntry {
    description: String,
    func: Box<dyn ToolFunction>,
}

pub struct ToolExecutor {
    tools: HashMap<String, ToolEntry>,
}

impl ToolExecutor {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register_tool(
        &mut self,
        name: String,
        description: String,
        func: Box<dyn ToolFunction>,
    ) {
        if self.tools.contains_key(&name) {
            println!("警告：工具 '{}' 已存在，将被覆盖。", name);
        }
        self.tools.insert(name.clone(), ToolEntry { description, func });
        println!("工具 '{}' 已注册。", name);
    }

    pub async fn execute(&self, name: &str, input: &str) -> Result<String> {
        if let Some(entry) = self.tools.get(name) {
            Ok(entry.func.call(input).await)
        } else {
            Err(anyhow::anyhow!("错误：未找到名为 '{}' 的工具。", name))
        }
    }

    pub fn available_tools(&self) -> String {
        self.tools
            .iter()
            .map(|(name, entry)| format!("- {}: {}", name, entry.description))
            .collect::<Vec<_>>()
            .join("\n")
    }
}