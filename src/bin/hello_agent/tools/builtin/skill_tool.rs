use crate::hello_agent::skills::loader::SkillLoader;
use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;
use std::collections::HashMap;

pub struct SkillTool {
    skill_loader: SkillLoader,
}

impl SkillTool {
    pub fn new(skill_loader: SkillLoader) -> Self {
        SkillTool { skill_loader }
    }
}

impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }
    fn description(&self) -> &str {
        "加载技能获取专业知识"
    }
    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        let skill_name = parameters
            .get("skill")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if skill_name.is_empty() {
            return ToolResponse::error("INVALID_PARAM", "必须指定技能名称");
        }
        match self.skill_loader.get_skill(skill_name) {
            Some(skill) => {
                let args = parameters
                    .get("args")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let content = skill.body.replace("$ARGUMENTS", args);
                let mut data = HashMap::new();
                data.insert("name".into(), serde_json::json!(&skill.name));
                data.insert("description".into(), serde_json::json!(&skill.description));
                data.insert("loaded".into(), serde_json::json!(true));
                ToolResponse::success(
                    format!(
                        "✅ 技能已加载：{}\n📝 描述：{}\n\n{}",
                        skill.name, skill.description, content
                    ),
                    data,
                )
            }
            None => {
                let available = self.skill_loader.list_skills().join(", ");
                ToolResponse::error(
                    "NOT_FOUND",
                    &format!("技能'{}'不存在。可用技能：{}", skill_name, available),
                )
            }
        }
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("skill", "string", "要加载的技能名称"),
            ToolParameter::optional("args", "string", "可选参数")
                .with_default(serde_json::json!("")),
        ]
    }
}
