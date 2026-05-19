use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// 技能数据
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
    pub path: PathBuf,
    pub dir: PathBuf,
}

/// 技能加载器
pub struct SkillLoader {
    skills_dir: PathBuf,
    skills_cache: HashMap<String, Skill>,
    metadata_cache: HashMap<String, HashMap<String, String>>,
}

impl SkillLoader {
    pub fn new(skills_dir: PathBuf) -> Self {
        fs::create_dir_all(&skills_dir).ok();

        let mut loader = Self {
            skills_dir,
            skills_cache: HashMap::new(),
            metadata_cache: HashMap::new(),
        };
        loader.scan_skills();
        loader
    }

    fn scan_skills(&mut self) {
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

                if let Ok(content) = fs::read_to_string(&skill_md) {
                    if let Some(metadata) = self.parse_frontmatter(&content) {
                        let name = metadata.get("name").cloned().unwrap_or_else(|| {
                            path.file_name().unwrap().to_string_lossy().to_string()
                        });

                        let mut info = HashMap::new();
                        info.insert("name".into(), name.clone());
                        info.insert(
                            "description".into(),
                            metadata.get("description").cloned().unwrap_or_default(),
                        );
                        info.insert("path".into(), skill_md.to_string_lossy().to_string());
                        info.insert("dir".into(), path.to_string_lossy().to_string());

                        self.metadata_cache.insert(name, info);
                    }
                }
            }
        }
    }

    fn parse_frontmatter(&self, content: &str) -> Option<HashMap<String, String>> {
        let content = content.trim();
        if !content.starts_with("---") {
            return None;
        }

        let rest = &content[3..];
        let end = rest.find("---")?;
        let yaml_str = &rest[..end].trim();

        // 简化 YAML 解析
        let mut map = HashMap::new();
        for line in yaml_str.lines() {
            if let Some((key, value)) = line.split_once(':') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        if map.contains_key("name") && map.contains_key("description") {
            Some(map)
        } else {
            None
        }
    }

    /// 获取所有技能描述
    pub fn get_descriptions(&self) -> String {
        if self.metadata_cache.is_empty() {
            return "（暂无可用技能）".into();
        }

        self.metadata_cache
            .iter()
            .map(|(name, info)| {
                format!(
                    "- {}: {}",
                    name,
                    info.get("description").map(|s| s.as_str()).unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 按需加载完整技能
    pub fn get_skill(&mut self, name: &str) -> Option<Skill> {
        if let Some(skill) = self.skills_cache.get(name) {
            return Some(skill.clone());
        }

        let info = self.metadata_cache.get(name)?;
        let path = PathBuf::from(info.get("path")?);
        let dir = PathBuf::from(info.get("dir")?);

        let content = fs::read_to_string(&path).ok()?;

        // 提取 body（跳过 frontmatter）
        let content = content.trim();
        let body = if content.starts_with("---") {
            let rest = &content[3..];
            if let Some(end) = rest.find("---") {
                rest[end + 3..].trim().to_string()
            } else {
                content.to_string()
            }
        } else {
            content.to_string()
        };

        let skill = Skill {
            name: name.to_string(),
            description: info.get("description").cloned().unwrap_or_default(),
            body,
            path,
            dir,
        };

        self.skills_cache.insert(name.to_string(), skill.clone());
        Some(skill)
    }

    /// 列出所有可用技能
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