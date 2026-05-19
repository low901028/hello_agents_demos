use std::collections::HashMap;
use std::path::PathBuf;

use crate::hello_agent::skills::loader::SkillLoader;
use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::response::ToolResponse;

/// 技能工具
pub struct SkillTool {
    skill_loader: std::sync::Mutex<SkillLoader>,
}

impl SkillTool {
    pub fn new(skill_loader: SkillLoader) -> Self {
        Self {
            skill_loader: std::sync::Mutex::new(skill_loader),
        }
    }
}

impl Tool for SkillTool {
    fn name(&self) -> &str { "Skill" }

    fn description(&self) -> &str {
        "加载技能获取专业知识。按需加载领域知识。"
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("skill", "string", "要加载的技能名称", true),
            ToolParameter::new("args", "string", "可选参数，替换 $ARGUMENTS 占位符", false)
                .with_default(serde_json::json!("")),
        ]
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let skill_name = parameters.get("skill").and_then(|v| v.as_str()).unwrap_or("");

        if skill_name.is_empty() {
            return ToolResponse::error(ToolErrorCode::INVALID_PARAM, "必须指定技能名称");
        }

        match self.skill_loader.lock() {
            Ok(mut loader) => {
                match loader.get_skill(skill_name) {
                    Some(skill) => {
                        ToolResponse::success(format!(
                            "✅ 技能已加载：{}\n📝 描述：{}\n\n{}",
                            skill.name, skill.description, skill.body
                        ))
                            .with_data("name", skill.name)
                            .with_data("loaded", true)
                    }
                    None => {
                        let available = loader.list_skills().join(", ");
                        ToolResponse::error(
                            ToolErrorCode::NOT_FOUND,
                            format!("技能 '{}' 不存在。可用技能：{}", skill_name, available),
                        )
                    }
                }
            }
            Err(e) => ToolResponse::error(ToolErrorCode::INTERNAL_ERROR, format!("加载技能失败: {}", e)),
        }
    }
}