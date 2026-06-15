use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 技能元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub dir: PathBuf,
}

/// 技能完整内容
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
    pub path: PathBuf,
    pub dir: PathBuf,
}

pub struct SkillLoader {
    skills_dir: PathBuf,
    metadata_cache: HashMap<String, SkillMeta>,
    skills_cache: HashMap<String, Skill>,
}

impl SkillLoader {
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        let skills_dir = skills_dir.into();
        fs::create_dir_all(&skills_dir).ok();

        let mut loader = Self {
            skills_dir,
            metadata_cache: HashMap::new(),
            skills_cache: HashMap::new(),
        };
        loader.scan_skills();
        loader
    }

    /// 扫描技能目录，仅加载元数据
    fn scan_skills(&mut self) {
        self.metadata_cache.clear();
        if let Ok(entries) = fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let skill_md = path.join("SKILL.md");
                if !skill_md.exists() {
                    continue;
                }
                if let Some(meta) = self.parse_frontmatter(&skill_md) {
                    self.metadata_cache.insert(meta.name.clone(), meta);
                }
            }
        }
    }

    /// 解析 YAML frontmatter，返回 SkillMeta
    fn parse_frontmatter(&self, path: &Path) -> Option<SkillMeta> {
        let content = fs::read_to_string(path).ok()?;
        let mut lines = content.lines();
        let mut yaml_lines = Vec::new();
        let mut in_frontmatter = false;
        let mut found = false;

        for line in lines {
            let trimmed = line.trim();
            if trimmed == "---" {
                if !in_frontmatter {
                    in_frontmatter = true;
                    continue;
                } else {
                    found = true;
                    break;
                }
            }
            if in_frontmatter {
                yaml_lines.push(trimmed);
            }
        }

        if !found {
            return None;
        }

        let yaml_str = yaml_lines.join("\n");
        let metadata: HashMap<String, String> = serde_yaml::from_str(&yaml_str).ok()?;
        let name = metadata.get("name")?.clone();
        let description = metadata.get("description")?.clone();
        if name.is_empty() || description.is_empty() {
            return None;
        }

        Some(SkillMeta {
            name,
            description,
            path: path.to_path_buf(),
            dir: path.parent()?.to_path_buf(),
        })
    }

    /// 获取所有技能的描述列表
    pub fn get_descriptions(&self) -> String {
        if self.metadata_cache.is_empty() {
            return "（暂无可用技能）".to_string();
        }
        self.metadata_cache
            .iter()
            .map(|(name, meta)| format!("- {}: {}", name, meta.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 按需加载完整技能
    pub fn get_skill(&mut self, name: &str) -> Option<&Skill> {
        if self.skills_cache.contains_key(name) {
            return self.skills_cache.get(name);
        }
        let meta = self.metadata_cache.get(name)?;
        let content = fs::read_to_string(&meta.path).ok()?;
        let body = Self::extract_body(&content)?;
        let skill = Skill {
            name: meta.name.clone(),
            description: meta.description.clone(),
            body,
            path: meta.path.clone(),
            dir: meta.dir.clone(),
        };
        self.skills_cache.insert(name.to_string(), skill);
        self.skills_cache.get(name)
    }

    /// 提取 SKILL.md 中 frontmatter 之后的内容
    fn extract_body(content: &str) -> Option<String> {
        let mut lines = content.lines();
        let mut in_frontmatter = false;
        let mut started = false;
        let mut body = String::new();

        for line in lines {
            let trimmed = line.trim();
            if trimmed == "---" {
                if !in_frontmatter {
                    in_frontmatter = true;
                    continue;
                } else {
                    started = true;
                    continue;
                }
            }
            if started {
                body.push_str(line);
                body.push('\n');
            }
        }

        if body.is_empty() {
            None
        } else {
            Some(body.trim().to_string())
        }
    }

    /// 列出所有可用技能名称
    pub fn list_skills(&self) -> Vec<String> {
        self.metadata_cache.keys().cloned().collect()
    }

    /// 热重载
    pub fn reload(&mut self) {
        self.skills_cache.clear();
        self.metadata_cache.clear();
        self.scan_skills();
    }
}
