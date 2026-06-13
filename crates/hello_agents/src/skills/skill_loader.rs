//! skills/skill_loader.rs
//! Skills 加载器
//!
//! 实现渐进式披露机制：
//! - Layer 1: Metadata（启动时加载，~100 tokens/skill）
//! - Layer 2: SKILL.md body（按需加载，~2000+ tokens）
//! - Layer 3: Resources（可选，按需）
use regex::bytes::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// 技能元数据（启动时加载）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub path: PathBuf, // 指向 SKILL.md 的路径
    pub dir: PathBuf,  // 技能目录
}

/// 技能数据类
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,       // SKILL.md 的 body 部分
    pub path: PathBuf,      // SKILL.md 路径
    pub dir: PathBuf,       // 技能目录
    // 以下为派生属性
    pub scripts: Vec<PathBuf>,
    pub examples: Vec<PathBuf>,
    pub references: Vec<PathBuf>,
}

impl Skill {
    /// 从 Meta 和完整内容构建 Skill
    fn from_meta_and_body(meta: &SkillMeta, body: String) -> Self {
        let scripts = Self::list_files(&meta.dir.join("scripts"));
        let examples = Self::list_files(&meta.dir.join("examples"));
        let references = Self::list_files(&meta.dir.join("references"));
        Skill {
            name: meta.name.clone(),
            description: meta.description.clone(),
            body,
            path: meta.path.clone(),
            dir: meta.dir.clone(),
            scripts,
            examples,
            references,
        }
    }

    fn list_files(dir: &Path) -> Vec<PathBuf> {
        if !dir.exists() || !dir.is_dir() {
            return vec![];
        }
        let mut files = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    files.push(path);
                }
            }
        }
        files
    }
}

/// 技能加载器
pub struct SkillLoader {
    skills_dir: PathBuf,
    skills_cache: HashMap<String, Skill>,           // 完整技能缓存
    metadata_cache: HashMap<String, SkillMeta>,     // 仅元数据缓存
}

impl SkillLoader {
    /// 创建新的技能加载器
    ///
    /// # Arguments
    /// * `skills_dir` - 技能目录路径
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        let skills_dir = skills_dir.into();
        // 确保目录存在
        fs::create_dir_all(&skills_dir).ok();

        let mut loader = Self {
            skills_dir,
            skills_cache: HashMap::new(),
            metadata_cache: HashMap::new(),
        };
        loader.scan_skills();
        loader
    }

    /// 扫描技能目录，只加载元数据
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
                if let Some(meta) = self.parse_frontmatter_only(&skill_md) {
                    self.metadata_cache.insert(meta.name.clone(), meta);
                }
            }
        }
    }

    /// 仅解析 YAML frontmatter，不加载 body
    fn parse_frontmatter_only(&self, path: &Path) -> Option<SkillMeta> {
        let content = fs::read_to_string(path).ok()?;
        // 匹配 YAML frontmatter，支持 Windows 换行
        let re = Regex::new(r"(?s)^---\s*\n(.*?)\n---\s*\n").unwrap();
        let caps = re.captures(content.as_bytes())?;
        let yaml_str = String::from_utf8_lossy(&caps[1]);
        let metadata: HashMap<String, String> = serde_yaml::from_str(&yaml_str).ok()?;
        let name = metadata.get("name")?.clone();
        let description = metadata.get("description")?.clone();
        // 验证必需字段
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

    /// 获取所有技能的元数据描述（用于系统提示词）
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
        let re = Regex::new(r"(?s)^---\s*\n(.*?)\n---\s*\n(.*)$").unwrap();
        let caps = re.captures(content.as_bytes())?;
        let body = String::from_utf8_lossy(&caps[2]).trim().to_string();
        let skill = Skill::from_meta_and_body(meta, body);
        self.skills_cache.insert(name.to_string(), skill);
        self.skills_cache.get(name)
    }

    /// 列出所有可用技能名称
    pub fn list_skills(&self) -> Vec<String> {
        self.metadata_cache.keys().cloned().collect()
    }

    /// 热重载：清空缓存并重新扫描
    pub fn reload(&mut self) {
        self.skills_cache.clear();
        self.metadata_cache.clear();
        self.scan_skills();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn setup_temp_skills_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("test_skills");
        fs::create_dir_all(&dir).ok();
        dir
    }

    fn create_skill_md(dir: &Path, name: &str, description: &str, body: &str) {
        let skill_dir = dir.join(name);
        fs::create_dir_all(&skill_dir).ok();
        let md_path = skill_dir.join("SKILL.md");
        let mut f = fs::File::create(md_path).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: {}", name).unwrap();
        writeln!(f, "description: {}", description).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "{}", body).unwrap();
    }

    #[test]
    fn test_basic_loading() {
        let dir = setup_temp_skills_dir();
        create_skill_md(&dir, "pdf", "PDF 处理", "这里是技能内容");
        let mut loader = SkillLoader::new(&dir);
        assert_eq!(loader.list_skills(), vec!["pdf"]);
        let desc = loader.get_descriptions();
        assert!(desc.contains("pdf"));
        let skill = loader.get_skill("pdf").unwrap();
        assert_eq!(skill.name, "pdf");
        assert_eq!(skill.body, "这里是技能内容");
        // 再次获取应该命中缓存
        assert!(loader.skills_cache.contains_key("pdf"));
    }

    #[test]
    fn test_reload() {
        let dir = setup_temp_skills_dir();
        create_skill_md(&dir, "pdf", "PDF 处理", "v1");
        let mut loader = SkillLoader::new(&dir);
        assert_eq!(loader.get_skill("pdf").unwrap().body, "v1");
        // 修改文件内容模拟热重载
        create_skill_md(&dir, "pdf", "PDF 处理", "v2");
        loader.reload();
        assert_eq!(loader.get_skill("pdf").unwrap().body, "v2");
    }

    #[test]
    fn test_missing_frontmatter_fields() {
        let dir = setup_temp_skills_dir();
        // 创建缺少 description 的 SKILL.md
        let skill_dir = dir.join("incomplete");
        fs::create_dir_all(&skill_dir).ok();
        let mut f = fs::File::create(skill_dir.join("SKILL.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: incomplete").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "body").unwrap();
        let loader = SkillLoader::new(&dir);
        assert!(loader.metadata_cache.is_empty());
    }
}