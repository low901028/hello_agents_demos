use crate::hello_agent::tools::base::{Tool, ToolParameter};
use crate::hello_agent::tools::error::ToolErrorCode;
use crate::hello_agent::tools::registry::ToolRegistry;
use crate::hello_agent::tools::response::ToolResponse;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub struct ReadTool {
    project_root: PathBuf,
    working_dir: PathBuf,
    registry: Option<std::sync::Arc<ToolRegistry>>,
}

impl ReadTool {
    pub fn new(
        project_root: &str,
        working_dir: Option<&str>,
        registry: Option<std::sync::Arc<ToolRegistry>>,
    ) -> Self {
        let root = PathBuf::from(project_root);
        let wd = working_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| root.clone());
        ReadTool {
            project_root: root,
            working_dir: wd,
            registry,
        }
    }

    fn list_directory(&self, path: &str, full_path: &Path) -> ToolResponse {
        match fs::read_dir(full_path) {
            Ok(entries) => {
                let items: Vec<HashMap<String, serde_json::Value>> = entries
                    .flatten()
                    .map(|e| {
                        let mut m = HashMap::new();
                        m.insert(
                            "name".into(),
                            serde_json::json!(e.file_name().to_string_lossy()),
                        );
                        m.insert(
                            "type".into(),
                            serde_json::json!(
                                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                    "directory"
                                } else {
                                    "file"
                                }
                            ),
                        );
                        m
                    })
                    .collect();
                let mut data = HashMap::new();
                data.insert("path".into(), serde_json::json!(path));
                data.insert("entries".into(), serde_json::json!(items));
                ToolResponse::success(format!("目录 '{}' 的内容", path), data)
            }
            Err(e) => ToolResponse::error(
                ToolErrorCode::AccessDenied.as_str(),
                &format!("无法访问: {}", e),
            ),
        }
    }
}

impl Tool for ReadTool {
    fn name(&self) -> &str {
        "Read"
    }
    fn description(&self) -> &str {
        "读取文件内容或列出目录内容"
    }

    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        let path = parameters
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if path.is_empty() {
            return ToolResponse::error("INVALID_PARAM", "缺少 path");
        }
        let full = self.working_dir.join(path);
        if !full.exists() {
            return ToolResponse::error("NOT_FOUND", &format!("路径'{}'不存在", path));
        }
        if full.is_dir() {
            return self.list_directory(path, &full);
        }
        match fs::read_to_string(&full) {
            Ok(content) => {
                let offset = parameters
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let limit = parameters
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2000) as usize;
                let lines: Vec<&str> = content.lines().collect();
                let total = lines.len();
                let start = offset.min(total);
                let end = (start + limit).min(total);
                let content = lines[start..end].join("\n");
                let mut data = HashMap::new();
                data.insert("content".into(), serde_json::json!(content));
                data.insert("lines".into(), serde_json::json!(end - start));
                data.insert("total_lines".into(), serde_json::json!(total));
                if let Ok(meta) = fs::metadata(&full) {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    data.insert("file_mtime_ms".into(), serde_json::json!(mtime));
                    data.insert("file_size_bytes".into(), serde_json::json!(meta.len()));
                }
                ToolResponse::success(format!("读取{}行(共{}行)", end - start, total), data)
            }
            Err(e) => ToolResponse::error("INTERNAL_ERROR", &format!("读取失败: {}", e)),
        }
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "文件或目录路径"),
            ToolParameter::optional("offset", "integer", "起始行号")
                .with_default(serde_json::json!(0)),
            ToolParameter::optional("limit", "integer", "最大行数")
                .with_default(serde_json::json!(2000)),
        ]
    }
}

pub struct WriteTool {
    working_dir: PathBuf,
}

impl WriteTool {
    pub fn new(
        _project_root: &str,
        working_dir: Option<&str>,
        _registry: Option<std::sync::Arc<ToolRegistry>>,
    ) -> Self {
        WriteTool {
            working_dir: working_dir
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
        }
    }
}

impl Tool for WriteTool {
    fn name(&self) -> &str {
        "Write"
    }
    fn description(&self) -> &str {
        "创建或覆盖文件"
    }
    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        let path = parameters
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let content = parameters
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if path.is_empty() {
            return ToolResponse::error("INVALID_PARAM", "缺少 path");
        }
        let full = self.working_dir.join(path);
        if let Some(p) = full.parent() {
            let _ = fs::create_dir_all(p);
        }
        match fs::write(&full, content) {
            Ok(_) => ToolResponse::success(
                format!("写入{} ({}字节)", path, content.len()),
                HashMap::new(),
            ),
            Err(e) => ToolResponse::error("INTERNAL_ERROR", &format!("写入失败: {}", e)),
        }
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "文件路径"),
            ToolParameter::new("content", "string", "文件内容"),
        ]
    }
}

pub struct EditTool {
    working_dir: PathBuf,
}

impl EditTool {
    pub fn new(
        _project_root: &str,
        working_dir: Option<&str>,
        _registry: Option<std::sync::Arc<ToolRegistry>>,
    ) -> Self {
        EditTool {
            working_dir: working_dir
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
        }
    }
}

impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }
    fn description(&self) -> &str {
        "精确替换文件内容"
    }
    fn run(&self, parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        let path = parameters
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let old = parameters
            .get("old_string")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let new = parameters
            .get("new_string")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if path.is_empty() {
            return ToolResponse::error("INVALID_PARAM", "缺少 path");
        }
        let full = self.working_dir.join(path);
        match fs::read_to_string(&full) {
            Ok(content) => {
                let matches = content.matches(old).count();
                if matches != 1 {
                    return ToolResponse::error(
                        "INVALID_PARAM",
                        &format!("old_string必须唯一匹配，找到{}处", matches),
                    );
                }
                match fs::write(&full, content.replace(old, new)) {
                    Ok(_) => ToolResponse::success(format!("编辑完成 {}", path), HashMap::new()),
                    Err(e) => ToolResponse::error("INTERNAL_ERROR", &format!("写入失败: {}", e)),
                }
            }
            Err(e) => ToolResponse::error("NOT_FOUND", &format!("文件不存在: {}", e)),
        }
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "文件路径"),
            ToolParameter::new("old_string", "string", "要替换的内容"),
            ToolParameter::new("new_string", "string", "替换后的内容"),
        ]
    }
}

pub struct MultiEditTool {
    working_dir: PathBuf,
}

impl MultiEditTool {
    pub fn new(
        _project_root: &str,
        working_dir: Option<&str>,
        _registry: Option<std::sync::Arc<ToolRegistry>>,
    ) -> Self {
        MultiEditTool {
            working_dir: working_dir
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
        }
    }
}

impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "MultiEdit"
    }
    fn description(&self) -> &str {
        "批量替换文件内容"
    }
    fn run(&self, _parameters: HashMap<String, serde_json::Value>) -> ToolResponse {
        ToolResponse::success("MultiEdit暂未完整实现", HashMap::new())
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "文件路径"),
            ToolParameter::new("edits", "array", "替换列表"),
        ]
    }
}
