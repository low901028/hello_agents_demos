//! src/tools/builtin/file.rs
//! 文件操作工具 - 支持乐观锁机制
//! 包含 ReadTool, WriteTool, EditTool, MultiEditTool

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::fs;
use std::path::{Path, PathBuf};
use serde_json::Value;
use crate::core::traits::tool::{Tool, ToolParameter};
use crate::core::types::exceptions::HelloAgentException;
use crate::tools::error::ToolErrorCode;
use crate::tools::registry::ToolRegistry;
use crate::tools::response::{ToolResponse, ToolStatus};
use crate::tools::tool_base::ToolBase;

pub struct ReadTool {
    base: ToolBase,
    working_dir: PathBuf,
    registry: Option<std::sync::Arc<std::sync::Mutex<crate::tools::registry::ToolRegistry>>>,
}

impl ReadTool {
    pub fn new(project_root: impl Into<PathBuf>, working_dir: Option<PathBuf>, registry: Option<std::sync::Arc<std::sync::Mutex<crate::tools::registry::ToolRegistry>>>) -> Self {
        let wd = working_dir.unwrap_or_else(|| project_root.into());
        Self { base: ToolBase::new("Read", "读取文件或列出目录内容", false), working_dir: wd, registry }
    }
}

impl Tool for ReadTool {
    fn name(&self) -> &str { &self.base.name }
    fn description(&self) -> &str { &self.base.description }
    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let full = self.working_dir.join(path);
        if !full.exists() { return Ok(ToolResponse::error(ToolErrorCode::NotFound.as_str(), "文件不存在", None, None)); }
        if full.is_dir() {
            // 简化目录列表
            let entries: Vec<_> = fs::read_dir(&full)?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
            return Ok(ToolResponse::success(format!("目录内容: {:?}", entries), None, None, None));
        }
        let content = fs::read_to_string(&full)?;
        let lines: Vec<&str> = content.lines().collect();
        let offset = parameters.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = parameters.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;
        let selected: Vec<&str> = lines.into_iter().skip(offset).take(limit).collect();
        Ok(ToolResponse::success(selected.join("\n"), None, None, None))
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "文件路径", true, None),
            ToolParameter::new("offset", "integer", "起始行", false, Some(Value::Number(0.into()))),
            ToolParameter::new("limit", "integer", "最大行数", false, Some(Value::Number(2000.into()))),
        ]
    }
    fn box_clone(&self) -> Box<dyn Tool> { Box::new(Self { base: self.base.clone(), working_dir: self.working_dir.clone(), registry: self.registry.clone() }) }
}

// ------------------------------------------------------------
// 辅助函数：跨平台兼容
// ------------------------------------------------------------
fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    for unit in UNITS {
        if size < 1024.0 {
            return format!("{:.1}{}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1}TB", size)
}

fn format_time(ms: i64) -> String {
    let secs = (ms / 1000) as i64;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
        dt.with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    } else {
        "?".to_string()
    }
}

fn backup_file(full_path: &Path) -> std::io::Result<PathBuf> {
    let backup_dir = full_path.parent().unwrap().join(".backups");
    fs::create_dir_all(&backup_dir)?;
    let now = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_name = format!(
        "{}.{}.bak",
        full_path.file_name().unwrap().to_str().unwrap(),
        now
    );
    let backup_path = backup_dir.join(backup_name);
    fs::copy(full_path, &backup_path)?;
    Ok(backup_path)
}

fn resolve_path(working_dir: &Path, path: &str) -> PathBuf {
    let path = path.replace('\\', "/");
    let p = PathBuf::from(&path);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}

// ------------------------------------------------------------
// WriteTool - 文件写入工具
// ------------------------------------------------------------
pub struct WriteTool {
    base: ToolBase,
    working_dir: PathBuf,
    registry: Option<Arc<Mutex<ToolRegistry>>>,
}

impl WriteTool {
    pub fn new(
        project_root: impl Into<PathBuf>,
        working_dir: Option<PathBuf>,
        registry: Option<Arc<Mutex<ToolRegistry>>>,
    ) -> Self {
        let working_dir = working_dir.unwrap_or_else(|| project_root.into());
        Self {
            base: ToolBase::new("Write", "创建或覆盖文件，支持冲突检测和原子写入", false),
            working_dir,
            registry,
        }
    }
}

impl Tool for WriteTool {
    fn name(&self) -> &str {
        &self.base.name
    }
    fn description(&self) -> &str {
        &self.base.description
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "文件路径（相对项目根目录）", true, None),
            ToolParameter::new("content", "string", "文件内容", true, None),
            ToolParameter::new(
                "file_mtime_ms",
                "integer",
                "缓存的文件修改时间（用于冲突检测）",
                false,
                None,
            ),
        ]
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let path = parameters
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let content = parameters
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let cached_mtime = parameters.get("file_mtime_ms").and_then(|v| v.as_i64());

        if path.is_empty() || content.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "缺少必需参数: path 或 content",
                None,
                None,
            ));
        }

        let full_path = resolve_path(&self.working_dir, path);
        let mut backup_path: Option<PathBuf> = None;

        if full_path.exists() {
            if let Ok(metadata) = fs::metadata(&full_path) {
                if let Ok(mtime) = metadata.modified() {
                    let current_mtime_ms = mtime
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64;
                    if let Some(cached) = cached_mtime {
                        if current_mtime_ms != cached {
                            return Ok(ToolResponse::error(
                                ToolErrorCode::Conflict.as_str(),
                                &format!(
                                    "文件自上次读取后被修改。当前 mtime={}, 缓存 mtime={}",
                                    current_mtime_ms, cached
                                ),
                                None,
                                Some(serde_json::json!({
                                    "current_mtime_ms": current_mtime_ms,
                                    "cached_mtime_ms": cached,
                                })),
                            ));
                        }
                    }
                }
            }
            backup_path = Some(
                backup_file(&full_path)
                    .map_err(|e| HelloAgentException::ToolException(format!("备份失败: {}", e)))?,
            );
        } else if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| HelloAgentException::ToolException(format!("创建目录失败: {}", e)))?;
        }

        // 原子写入
        let temp_path = full_path.with_extension("tmp");
        fs::write(&temp_path, content)
            .map_err(|e| HelloAgentException::ToolException(format!("写入临时文件失败: {}", e)))?;
        fs::rename(&temp_path, &full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("原子重命名失败: {}", e)))?;

        let size_bytes = content.len() as u64;
        let backup_rel = backup_path.as_ref().map(|p| {
            p.strip_prefix(&self.working_dir)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        });

        Ok(ToolResponse::success(
            format!("成功写入 {} ({} 字节)", path, size_bytes),
            Some(serde_json::json!({
                "written": true,
                "size_bytes": size_bytes,
                "backup_path": backup_rel,
            })),
            None,
            None,
        ))
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
            working_dir: self.working_dir.clone(),
            registry: self.registry.clone(),
        })
    }
}

// ------------------------------------------------------------
// EditTool - 文件编辑工具
// ------------------------------------------------------------
pub struct EditTool {
    base: ToolBase,
    working_dir: PathBuf,
    registry: Option<Arc<Mutex<ToolRegistry>>>,
}

impl EditTool {
    pub fn new(
        project_root: impl Into<PathBuf>,
        working_dir: Option<PathBuf>,
        registry: Option<Arc<Mutex<ToolRegistry>>>,
    ) -> Self {
        let working_dir = working_dir.unwrap_or_else(|| project_root.into());
        Self {
            base: ToolBase::new("Edit", "精确替换文件内容，支持冲突检测和自动备份", false),
            working_dir,
            registry,
        }
    }
}

impl Tool for EditTool {
    fn name(&self) -> &str {
        &self.base.name
    }
    fn description(&self) -> &str {
        &self.base.description
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "要编辑的文件路径", true, None),
            ToolParameter::new(
                "old_string",
                "string",
                "要替换的内容（必须唯一匹配）",
                true,
                None,
            ),
            ToolParameter::new("new_string", "string", "替换后的内容", true, None),
            ToolParameter::new(
                "file_mtime_ms",
                "integer",
                "缓存的文件修改时间",
                false,
                None,
            ),
        ]
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
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
        let cached_mtime = parameters.get("file_mtime_ms").and_then(|v| v.as_i64());

        if path.is_empty() || old.is_empty() || new.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "缺少必需参数",
                None,
                None,
            ));
        }

        let full_path = resolve_path(&self.working_dir, path);
        if !full_path.exists() {
            return Ok(ToolResponse::error(
                ToolErrorCode::NotFound.as_str(),
                &format!("文件 '{}' 不存在", path),
                None,
                None,
            ));
        }

        // 冲突检测
        if let Ok(metadata) = fs::metadata(&full_path) {
            if let Ok(mtime) = metadata.modified() {
                let current_ms = mtime
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;
                if let Some(cached) = cached_mtime {
                    if current_ms != cached {
                        return Ok(ToolResponse::error(
                            ToolErrorCode::Conflict.as_str(),
                            &format!(
                                "文件自上次读取后被修改。当前 mtime={}, 缓存 mtime={}",
                                current_ms, cached
                            ),
                            None,
                            Some(serde_json::json!({
                                "current_mtime_ms": current_ms,
                                "cached_mtime_ms": cached,
                            })),
                        ));
                    }
                }
            }
        }

        let content = fs::read_to_string(&full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("读取文件失败: {}", e)))?;
        let matches = content.match_indices(old).count();
        if matches != 1 {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                &format!("old_string 必须唯一匹配。找到 {} 处匹配。", matches),
                Some(serde_json::json!({"matches": matches})),
                None,
            ));
        }

        let new_content = content.replace(old, new);
        let backup_path = backup_file(&full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("备份失败: {}", e)))?;
        fs::write(&full_path, &new_content)
            .map_err(|e| HelloAgentException::ToolException(format!("写入文件失败: {}", e)))?;

        let changed_bytes = (new.len() as i64) - (old.len() as i64);
        let backup_rel = backup_path
            .strip_prefix(&self.working_dir)
            .unwrap_or(&backup_path)
            .to_string_lossy()
            .into_owned();

        Ok(ToolResponse::success(
            format!("成功编辑 {} (变化 {:+} 字节)", path, changed_bytes),
            Some(serde_json::json!({
                "modified": true,
                "changed_bytes": changed_bytes,
                "backup_path": backup_rel,
            })),
            None,
            None,
        ))
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
            working_dir: self.working_dir.clone(),
            registry: self.registry.clone(),
        })
    }
}

// ------------------------------------------------------------
// MultiEditTool - 批量编辑工具
// ------------------------------------------------------------
pub struct MultiEditTool {
    base: ToolBase,
    working_dir: PathBuf,
    registry: Option<Arc<Mutex<ToolRegistry>>>,
}

impl MultiEditTool {
    pub fn new(
        project_root: impl Into<PathBuf>,
        working_dir: Option<PathBuf>,
        registry: Option<Arc<Mutex<ToolRegistry>>>,
    ) -> Self {
        let working_dir = working_dir.unwrap_or_else(|| project_root.into());
        Self {
            base: ToolBase::new("MultiEdit", "批量替换文件内容，支持原子性和冲突检测", false),
            working_dir,
            registry,
        }
    }
}

impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        &self.base.name
    }
    fn description(&self) -> &str {
        &self.base.description
    }
    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "要编辑的文件路径", true, None),
            ToolParameter::new(
                "edits",
                "array",
                "替换列表，每项包含 old_string 和 new_string",
                true,
                None,
            ),
            ToolParameter::new(
                "file_mtime_ms",
                "integer",
                "缓存的文件修改时间",
                false,
                None,
            ),
        ]
    }

    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let path = parameters
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let edits = parameters
            .get("edits")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let cached_mtime = parameters.get("file_mtime_ms").and_then(|v| v.as_i64());

        if path.is_empty() || edits.is_empty() {
            return Ok(ToolResponse::error(
                ToolErrorCode::InvalidParam.as_str(),
                "缺少必需参数",
                None,
                None,
            ));
        }

        let full_path = resolve_path(&self.working_dir, path);
        if !full_path.exists() {
            return Ok(ToolResponse::error(
                ToolErrorCode::NotFound.as_str(),
                &format!("文件 '{}' 不存在", path),
                None,
                None,
            ));
        }

        // 冲突检测
        if let Ok(metadata) = fs::metadata(&full_path) {
            if let Ok(mtime) = metadata.modified() {
                let current_ms = mtime
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;
                if let Some(cached) = cached_mtime {
                    if current_ms != cached {
                        return Ok(ToolResponse::error(
                            ToolErrorCode::Conflict.as_str(),
                            &format!(
                                "文件自上次读取后被修改。所有替换已取消。当前 mtime={}, 缓存 mtime={}",
                                current_ms, cached
                            ),
                            None,
                            Some(serde_json::json!({
                                "current_mtime_ms": current_ms,
                                "cached_mtime_ms": cached,
                            })),
                        ));
                    }
                }
            }
        }

        let mut content = fs::read_to_string(&full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("读取文件失败: {}", e)))?;
        let original_content = content.clone();

        // 验证所有替换
        for (i, edit) in edits.iter().enumerate() {
            let old = edit
                .get("old_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new = edit
                .get("new_string")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if old.is_empty() || new.is_empty() {
                return Ok(ToolResponse::error(
                    ToolErrorCode::InvalidParam.as_str(),
                    &format!("编辑项 {} 缺少 old_string 或 new_string", i),
                    None,
                    None,
                ));
            }
            let matches = content.match_indices(old).count();
            if matches != 1 {
                return Ok(ToolResponse::error(
                    ToolErrorCode::InvalidParam.as_str(),
                    &format!(
                        "编辑项 {}: old_string 必须唯一匹配。找到 {} 处匹配。",
                        i, matches
                    ),
                    Some(serde_json::json!({"edit_index": i, "matches": matches})),
                    None,
                ));
            }
        }

        // 执行替换
        for edit in &edits {
            let old = edit
                .get("old_string")
                .and_then(|v| v.as_str())
                .unwrap();
            let new = edit
                .get("new_string")
                .and_then(|v| v.as_str())
                .unwrap();
            content = content.replace(old, new);
        }

        let backup_path = backup_file(&full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("备份失败: {}", e)))?;
        fs::write(&full_path, &content)
            .map_err(|e| HelloAgentException::ToolException(format!("写入文件失败: {}", e)))?;

        let changed_bytes = (content.len() as i64) - (original_content.len() as i64);
        let backup_rel = backup_path
            .strip_prefix(&self.working_dir)
            .unwrap_or(&backup_path)
            .to_string_lossy()
            .into_owned();

        Ok(ToolResponse::success(
            format!(
                "成功执行 {} 个替换操作 (变化 {:+} 字节)",
                edits.len(),
                changed_bytes
            ),
            Some(serde_json::json!({
                "modified": true,
                "num_edits": edits.len(),
                "changed_bytes": changed_bytes,
                "backup_path": backup_rel,
            })),
            None,
            None,
        ))
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
            working_dir: self.working_dir.clone(),
            registry: self.registry.clone(),
        })
    }
}