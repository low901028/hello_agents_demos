//! skill
//! 技能工具 - 允许 Agent 按需加载领域知识

use std::path::PathBuf;
///
/// 特性：
/// - 渐进式披露：仅在需要时加载完整技能
/// - 缓存友好：作为 tool_result 注入，不修改 system_prompt
/// - 资源提示：自动列出可用的脚本、文档、示例等
/// - 参数替换：支持 $ARGUMENTS 占位符

use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::core::traits::tool::{Tool, ToolParameter};
use crate::core::types::exceptions::HelloAgentException;
use crate::tools::error::ToolErrorCode;
use crate::tools::response::{ToolResponse, ToolStatus};
use crate::tools::tool_base::ToolBase;
use crate::skills::skill_loader::Skill; // 引入新的 Skill 类型
use crate::skills::skill_loader::SkillLoader; // 引入新的 SkillLoader 结构体

// ------------------------------------------------------------
// SkillTool
// ------------------------------------------------------------
/// 技能工具
///
/// 允许模型按需加载领域知识。
pub struct SkillTool {
    base: ToolBase,
    skill_loader: Arc<Mutex<SkillLoader>>,
}

impl SkillTool {
    /// 初始化技能工具
    ///
    /// # Arguments
    /// * `skill_loader` - 技能加载器实例（已包装为 Arc<Mutex<SkillLoader>>）
    pub fn new(skill_loader: Arc<Mutex<SkillLoader>>) -> Self {
        let descriptions = skill_loader.lock().unwrap().get_descriptions();

        let description = format!(
            "加载技能获取专业知识。\n\n可用技能：\n{}\n\n何时使用：\n- 任务明确匹配某个技能描述时，立即使用\n- 开始领域特定工作之前\n- 需要模型不具备的专业知识时\n\n注意：加载技能后，请严格遵循技能说明来完成用户任务。",
            descriptions
        );

        Self {
            base: ToolBase::new("Skill", description, false),
            skill_loader,
        }
    }

    /// 生成资源提示文本
    fn get_resources_hint(&self, skill: &Skill) -> String {
        let mut resources = Vec::new();

        let folders = [
            ("scripts", "脚本"),
            ("references", "参考文档"),
            ("assets", "资源"),
            ("examples", "示例"),
        ];

        for (folder, label) in folders {
            // 直接使用 skill 中的资源列表，避免重复扫描目录
            let files: &[PathBuf] = match folder {
                "scripts" => &skill.scripts,
                "references" => &skill.references,
                "assets" => &[], // 如有需要可扩展
                "examples" => &skill.examples,
                _ => &[],
            };

            if !files.is_empty() {
                let file_list: Vec<String> = files
                    .iter()
                    .take(5)
                    .map(|f| f.file_name().unwrap_or_default().to_string_lossy().into_owned())
                    .collect();
                let mut hint = format!("  - {}：{}", label, file_list.join(", "));
                if files.len() > 5 {
                    hint.push_str(&format!(" 等 {} 个文件", files.len()));
                }
                resources.push(hint);
            }
        }

        if resources.is_empty() {
            String::new()
        } else {
            format!("\n\n**可用资源**：\n{}", resources.join("\n"))
        }
    }
}

impl Tool for SkillTool {
    fn name(&self) -> &str {
        &self.base.name
    }

    fn description(&self) -> &str {
        &self.base.description
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let skill_name = parameters.get("skill").and_then(|v| v.as_str()).unwrap_or("");
        let args = parameters.get("args").and_then(|v| v.as_str()).unwrap_or("");

        if skill_name.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "必须指定技能名称",
                None,
                Some(serde_json::json!({"params_input": parameters})),
            ));
        }

        let mut loader = self.skill_loader.lock().unwrap();

        match loader.get_skill(skill_name) {
            None => {
                let available = loader.list_skills().join(", ");
                Ok(ToolResponse::error(
                    ToolErrorCode::NotFound.as_str(),
                    &format!(
                        "技能 '{}' 不存在。可用技能：{}",
                        skill_name, available
                    ),
                    None,
                    Some(serde_json::json!({
                        "params_input": parameters,
                        "available_skills": loader.list_skills()
                    })),
                ))
            }
            Some(skill) => {
                // 替换 $ARGUMENTS 占位符
                let content = skill.body.replace("$ARGUMENTS", args);

                // 列出可用资源
                let resources_hint = self.get_resources_hint(&skill);

                // 构造完整技能内容
                let full_content = format!(
                    r#"<skill-loaded name="{}">
{}
{}
</skill-loaded>

✅ 技能已加载：{}
📝 描述：{}

请严格遵循上述技能说明来完成用户任务。"#,
                    skill_name,
                    content,
                    resources_hint,
                    skill.name,
                    skill.description
                );
                let len = full_content.len();
                Ok(ToolResponse::success(
                    full_content,
                    Some(serde_json::json!({
                        "name": skill.name,
                        "description": skill.description,
                        "loaded": true,
                        "token_estimate": len,
                        "has_resources": !resources_hint.is_empty()
                    })),
                    None,
                    None,
                ))
            }
        }
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new(
                "skill",
                "string",
                "要加载的技能名称",
                true,
                None,
            ),
            ToolParameter::new(
                "args",
                "string",
                "可选参数，将替换 SKILL.md 中的 $ARGUMENTS 占位符",
                false,
                Some(Value::String(String::new())),
            ),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
            skill_loader: Arc::clone(&self.skill_loader),
        })
    }
}

// ------------------------------------------------------------
// 测试用例（使用真实 SkillLoader）
// ------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use crate::tools::registry::{global_registry, ToolRegistry};

    fn set_up_registry() -> std::sync::MutexGuard<'static, ToolRegistry>{
        // 全局注册中心
        let guard = global_registry();
        let registry = guard.lock().unwrap();

        registry
    }

    /// 创建临时技能目录，并写入一个 SKILL.md
    fn setup_skill_dir() -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join("skill_tool_test");
        let _ = fs::remove_dir_all(&dir); // 清理旧数据
        fs::create_dir_all(&dir).unwrap();

        let skill_dir = dir.join("pdf");
        fs::create_dir_all(&skill_dir).unwrap();

        let mut f = fs::File::create(skill_dir.join("SKILL.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: pdf").unwrap();
        writeln!(f, "description: 处理 PDF 文件").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "使用 pdf 库操作 $ARGUMENTS").unwrap();

        // 创建资源目录
        fs::create_dir(skill_dir.join("scripts")).ok();
        let mut script_file = fs::File::create(skill_dir.join("scripts/run.py")).unwrap();
        script_file.write_all(b"print('hello')").unwrap();

        (dir, skill_dir)
    }

    #[test]
    fn test_load_existing_skill() {
        let (tmp_dir, _) = setup_skill_dir();
        let loader = SkillLoader::new(tmp_dir.clone());
        let tool = SkillTool::new(Arc::new(Mutex::new(loader)));

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = serde_json::json!({"skill": "pdf", "args": "output.pdf"});
        let resp = registry.execute_tool("Skill", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================

        //let resp = tool.run(serde_json::json!({"skill": "pdf", "args": "output.pdf"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        let text = &resp.text;
        assert!(text.contains("技能已加载：pdf"));
        assert!(text.contains("output.pdf"));
        let data = resp.data;
        assert_eq!(data["loaded"], true);
        assert!(data["token_estimate"].as_u64().unwrap() > 0);
        assert_eq!(data["has_resources"], true);
        // 清理
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_missing_skill_returns_error() {
        let (tmp_dir, _) = setup_skill_dir();
        let loader = SkillLoader::new(tmp_dir.clone());
        let tool = SkillTool::new(Arc::new(Mutex::new(loader)));

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = serde_json::json!({"skill": "unknown"});
        let resp = registry.execute_tool("Skill", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================

        //let resp = tool.run(serde_json::json!({"skill": "unknown"})).unwrap();
        assert_eq!(resp.status, ToolStatus::Error);
        assert_eq!(
            resp.error_info.as_ref().unwrap().code,
            ToolErrorCode::NotFound.as_str()
        );
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_empty_skill_name() {
        let (tmp_dir, _) = setup_skill_dir();
        let loader = SkillLoader::new(&tmp_dir);
        let tool = SkillTool::new(Arc::new(Mutex::new(loader)));

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = serde_json::json!({"skill": ""});
        let resp = registry.execute_tool("Skill", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================

        //let resp = tool.run(serde_json::json!({"skill": ""})).unwrap();
        assert_eq!(resp.status, ToolStatus::Error);
        assert_eq!(
            resp.error_info.as_ref().unwrap().code,
            ToolErrorCode::InvalidParam.as_str()
        );
        let _ = fs::remove_dir_all(&tmp_dir);
    }
}