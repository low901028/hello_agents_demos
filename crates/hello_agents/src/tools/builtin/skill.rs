use crate::core::traits::tool::Tool;
use crate::core::types::exceptions::HelloAgentError;
use crate::core::types::response::ToolResponse;
use crate::skills::skill_loader::SkillLoader;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, Mutex};

pub struct SkillTool {
    loader: Arc<Mutex<SkillLoader>>,
}

impl SkillTool {
    pub fn new(loader: Arc<Mutex<SkillLoader>>) -> Self {
        Self { loader }
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }
    fn description(&self) -> &str {
        "加载技能获取专业知识"
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill": { "type": "string", "description": "要加载的技能名称" },
                "args": { "type": "string", "description": "可选参数，替换 $ARGUMENTS 占位符", "default": "" }
            },
            "required": ["skill"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResponse, HelloAgentError> {
        let skill_name = args["skill"].as_str().unwrap_or("");
        let args_str = args.get("args").and_then(|v| v.as_str()).unwrap_or("");
        if skill_name.is_empty() {
            return Ok(ToolResponse::error("INVALID_PARAM", "技能名称不能为空"));
        }

        let mut loader = self.loader.lock().unwrap();
        match loader.get_skill(skill_name) {
            Some(skill) => {
                let content = skill.body.replace("$ARGUMENTS", args_str);
                let full = format!(
                    "<skill-loaded name=\"{}\">\n{}\n</skill-loaded>",
                    skill_name, content
                );
                Ok(ToolResponse::success(full))
            }
            None => Ok(ToolResponse::error(
                "NOT_FOUND",
                &format!("技能 '{}' 不存在", skill_name),
            )),
        }
    }
}
