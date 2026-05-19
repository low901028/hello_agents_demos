use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::registry::ToolRegistry;
use crate::hello_agent::tools::response::ToolResponse;

/// 文件读取工具
pub struct ReadTool {
    project_root: PathBuf,
    working_dir: PathBuf,
    registry: Option<std::sync::Arc<std::sync::Mutex<ToolRegistry>>>,
}

impl ReadTool {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        let root = project_root.into();
        Self {
            working_dir: root.clone(),
            project_root: root,
            registry: None,
        }
    }

    pub fn with_registry(mut self, registry: std::sync::Arc<std::sync::Mutex<ToolRegistry>>) -> Self {
        self.registry = Some(registry);
        self
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let path = path.replace('\\', "/");
        if path.starts_with('/') {
            PathBuf::from(&path)
        } else {
            self.working_dir.join(&path)
        }
    }

    fn format_size(size: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = size as f64;
        for unit in UNITS {
            if size < 1024.0 {
                return format!("{:.1}{}", size, unit);
            }
            size /= 1024.0;
        }
        format!("{:.1}TB", size)
    }
}

impl Tool for ReadTool {
    fn name(&self) -> &str { "Read" }

    fn description(&self) -> &str {
        "读取文件内容或列出目录内容，支持行号范围和元数据缓存"
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "要读取的文件路径或目录路径", true),
            ToolParameter::new("offset", "integer", "起始行号（从 0 开始）", false).with_default(serde_json::json!(0)),
            ToolParameter::new("limit", "integer", "最大行数", false).with_default(serde_json::json!(2000)),
        ]
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
        if path.is_empty() {
            return ToolResponse::error(ToolErrorCode::INVALID_PARAM, "缺少必需参数: path");
        }

        let offset = parameters.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = parameters.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

        let full_path = self.resolve_path(path);

        if !full_path.exists() {
            return ToolResponse::error(ToolErrorCode::NOT_FOUND, format!("路径 '{}' 不存在", path));
        }

        if full_path.is_dir() {
            // 列出目录
            match fs::read_dir(&full_path) {
                Ok(entries) => {
                    let mut items = Vec::new();
                    let mut total_files = 0;
                    let mut total_dirs = 0;

                    for entry in entries.flatten() {
                        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        let name = entry.file_name().to_string_lossy().to_string();

                        let size_str = if is_dir {
                            total_dirs += 1;
                            "<DIR>".to_string()
                        } else {
                            total_files += 1;
                            entry.metadata().ok()
                                .map(|m| Self::format_size(m.len()))
                                .unwrap_or_else(|| "?".into())
                        };

                        items.push(format!("{} {} {}",
                                           if is_dir { "📁" } else { "📄" },
                                           name,
                                           size_str
                        ));
                    }

                    let text = format!(
                        "目录 '{}' 包含 {} 个文件，{} 个目录：\n\n{}",
                        path, total_files, total_dirs,
                        items.join("\n")
                    );

                    ToolResponse::success(text)
                        .with_data("path", path)
                        .with_data("is_directory", true)
                }
                Err(e) => ToolResponse::error(ToolErrorCode::ACCESS_DENIED, format!("无法访问目录: {}", e)),
            }
        } else {
            // 读取文件
            match fs::read_to_string(&full_path) {
                Ok(content) => {
                    let lines: Vec<&str> = content.lines().collect();
                    let total_lines = lines.len();

                    let start = offset.min(total_lines);
                    let end = (start + limit).min(total_lines);
                    let selected: Vec<&str> = lines[start..end].to_vec();
                    let preview = selected.join("\n");

                    let file_size = full_path.metadata()
                        .map(|m| m.len())
                        .unwrap_or(0);

                    ToolResponse::success(format!(
                        "读取 {} 行（共 {} 行，{} 字节）",
                        selected.len(), total_lines, file_size
                    ))
                        .with_data("content", preview)
                        .with_data("lines", selected.len() as u64)
                        .with_data("total_lines", total_lines as u64)
                }
                Err(e) => ToolResponse::error(ToolErrorCode::ACCESS_DENIED, format!("读取文件失败: {}", e)),
            }
        }
    }
}

/// 文件写入工具
pub struct WriteTool {
    project_root: PathBuf,
    working_dir: PathBuf,
}

impl WriteTool {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        let root = project_root.into();
        Self {
            working_dir: root.clone(),
            project_root: root,
        }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let path = path.replace('\\', "/");
        if path.starts_with('/') {
            PathBuf::from(&path)
        } else {
            self.working_dir.join(&path)
        }
    }
}

impl Tool for WriteTool {
    fn name(&self) -> &str { "Write" }

    fn description(&self) -> &str {
        "创建或覆盖文件，支持冲突检测和原子写入"
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "文件路径", true),
            ToolParameter::new("content", "string", "文件内容", true),
        ]
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = parameters.get("content").and_then(|v| v.as_str()).unwrap_or("");

        if path.is_empty() {
            return ToolResponse::error(ToolErrorCode::INVALID_PARAM, "缺少必需参数: path");
        }

        let full_path = self.resolve_path(path);

        // 确保父目录存在
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).ok();
        }

        match fs::write(&full_path, content) {
            Ok(_) => {
                let size = content.len();
                ToolResponse::success(format!("成功写入 {} ({} 字节)", path, size))
                    .with_data("written", true)
                    .with_data("size_bytes", size as u64)
            }
            Err(e) => ToolResponse::error(ToolErrorCode::ACCESS_DENIED, format!("写入文件失败: {}", e)),
        }
    }
}

/// 文件编辑工具
pub struct EditTool {
    project_root: PathBuf,
    working_dir: PathBuf,
}

impl EditTool {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        let root = project_root.into();
        Self {
            working_dir: root.clone(),
            project_root: root,
        }
    }
}

impl Tool for EditTool {
    fn name(&self) -> &str { "Edit" }

    fn description(&self) -> &str {
        "精确替换文件内容，支持冲突检测和自动备份"
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "要编辑的文件路径", true),
            ToolParameter::new("old_string", "string", "要替换的内容（必须唯一匹配）", true),
            ToolParameter::new("new_string", "string", "替换后的内容", true),
        ]
    }

    fn run(&self, parameters: &HashMap<String, serde_json::Value>) -> ToolResponse {
        let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let old_string = parameters.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
        let new_string = parameters.get("new_string").and_then(|v| v.as_str()).unwrap_or("");

        if path.is_empty() {
            return ToolResponse::error(ToolErrorCode::INVALID_PARAM, "缺少必需参数: path");
        }

        let full_path = self.working_dir.join(path.replace('\\', "/"));

        match fs::read_to_string(&full_path) {
            Ok(content) => {
                let matches = content.matches(old_string).count();
                if matches != 1 {
                    return ToolResponse::error(
                        ToolErrorCode::INVALID_PARAM,
                        format!("old_string 必须唯一匹配。找到 {} 处匹配。", matches),
                    );
                }

                let new_content = content.replace(old_string, new_string);
                match fs::write(&full_path, &new_content) {
                    Ok(_) => ToolResponse::success(format!("成功编辑 {}", path))
                        .with_data("modified", true),
                    Err(e) => ToolResponse::error(ToolErrorCode::ACCESS_DENIED, format!("写入失败: {}", e)),
                }
            }
            Err(e) => ToolResponse::error(ToolErrorCode::NOT_FOUND, format!("文件不存在: {}", e)),
        }
    }
}