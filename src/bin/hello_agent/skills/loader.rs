use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
    pub path: PathBuf,
    pub dir: PathBuf,
}

#[derive(Debug, Clone)]
struct SkillMetadata {
    name: String,
    description: String,
    path: PathBuf,
    dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SkillLoader {
    skills_dir: PathBuf,
    skills_cache: HashMap<String, Skill>,
    metadata_cache: HashMap<String, SkillMetadata>,
}

impl SkillLoader {
    pub fn new(skills_dir: PathBuf) -> Self {
        fs::create_dir_all(&skills_dir).ok();
        let mut loader = SkillLoader {
            skills_dir,
            skills_cache: HashMap::new(),
            metadata_cache: HashMap::new(),
        };
        loader.scan();
        loader
    }

    fn scan(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let dir = entry.path();
                if !dir.is_dir() {
                    continue;
                }
                let md = dir.join("SKILL.md");
                if !md.exists() {
                    continue;
                }
                if let Some(meta) = self.parse_frontmatter(&md) {
                    let name = meta
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&entry.file_name().to_string_lossy())
                        .to_string();
                    let desc = meta
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    self.metadata_cache.insert(
                        name.clone(),
                        SkillMetadata {
                            name,
                            description: desc,
                            path: md,
                            dir,
                        },
                    );
                }
            }
        }
    }

    fn parse_frontmatter(&self, path: &Path) -> Option<HashMap<String, serde_yaml::Value>> {
        let content = fs::read_to_string(path).ok()?;
        if !content.starts_with("---") {
            return None;
        }
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return None;
        }
        serde_yaml::from_str(parts[1].trim()).ok()
    }

    pub fn get_descriptions(&self) -> String {
        if self.metadata_cache.is_empty() {
            return "（暂无可用技能）".into();
        }
        self.metadata_cache
            .iter()
            .map(|(n, s)| format!("- {}: {}", n, s.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn get_skill(&self, name: &str) -> Option<Skill> {
        if let Some(s) = self.skills_cache.get(name) {
            return Some(s.clone());
        }
        let meta = self.metadata_cache.get(name)?;
        let content = fs::read_to_string(&meta.path).ok()?;
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return None;
        }
        let body = parts[2].trim().to_string();
        Some(Skill {
            name: meta.name.clone(),
            description: meta.description.clone(),
            body,
            path: meta.path.clone(),
            dir: meta.dir.clone(),
        })
    }

    pub fn list_skills(&self) -> Vec<String> {
        self.metadata_cache.keys().cloned().collect()
    }
    pub fn skill_count(&self) -> usize {
        self.metadata_cache.len()
    }
    pub fn reload(&mut self) {
        self.skills_cache.clear();
        self.metadata_cache.clear();
        self.scan();
    }
}
