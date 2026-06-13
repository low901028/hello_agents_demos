//! file_tools.rs
//! 文件操作工具 - 支持乐观锁机制
//! 文件操作工具 - 支持乐观锁机制
//!
//! 提供标准的文件读写编辑能力：
//! - ReadTool: 读取文件 + 元数据缓存
//! - WriteTool: 写入文件 + 冲突检测 + 原子写入
//! - EditTool: 精确替换 + 冲突检测 + 备份
//! - MultiEditTool: 批量替换 + 原子性保证

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Local};
use serde_json::Value;

use crate::core::exceptions::HelloAgentException;
use crate::tools::tool_base::{Tool, ToolBase, ToolParameter};
use crate::tools::tool_error::ToolErrorCode;
use crate::tools::tool_registry::ToolRegistry;
use crate::tools::tool_response::{ToolResponse, ToolStatus};

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
    if let Some(dt) = DateTime::from_timestamp(secs, nsecs) {
        dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        "?".to_string()
    }
}

fn backup_file(full_path: &Path) -> std::io::Result<PathBuf> {
    let backup_dir = full_path.parent().unwrap().join(".backups");
    fs::create_dir_all(&backup_dir)?;
    let now = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_name = format!("{}.{}.bak", full_path.file_name().unwrap().to_str().unwrap(), now);
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
// ReadTool
// ------------------------------------------------------------
///文件读取工具
///
///     功能：
///     - 读取文件内容（支持 offset/limit）
///     - 列出目录内容（当 path 是目录时）
///     - 自动获取文件元数据（mtime, size）
///     - 缓存元数据到 ToolRegistry（用于乐观锁）
///     - 跨平台兼容（Windows/Linux）
///
///     参数：
///     - path: 文件或目录路径（相对于 project_root）
///     - offset: 起始行号（可选，默认 0，仅文件有效）
///     - limit: 最大行数（可选，默认 2000，仅文件有效）
pub struct ReadTool {
    base: ToolBase,
    project_root: PathBuf,
    working_dir: PathBuf,
    registry: Option<Arc<Mutex<ToolRegistry>>>,
}

impl ReadTool {
    pub fn new(
        project_root: impl Into<PathBuf>,
        working_dir: Option<PathBuf>,
        registry: Option<Arc<Mutex<ToolRegistry>>>,
    ) -> Self {
        let project_root = project_root.into();
        let project_root = project_root.canonicalize().unwrap_or_else(|_| project_root.into());
        let default_working = working_dir.clone().unwrap_or_else(|| project_root.clone());
        let working_dir = working_dir
            .unwrap_or_else(|| project_root.clone())
            .canonicalize()
            .unwrap_or(default_working);
        Self {
            base: ToolBase::new("Read", "读取文件内容或列出目录内容，支持行号范围和元数据缓存", false),
            project_root,
            working_dir,
            registry,
        }
    }
}

impl Tool for ReadTool {
    fn name(&self) -> &str { &self.base.name }
    fn base(&self) -> &ToolBase { &self.base }
    fn base_mut(&mut self) -> &mut ToolBase { &mut self.base }
    /// 执行文件读取或目录列表
    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let offset = parameters.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = parameters.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

        if path.is_empty() {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "缺少必需参数: path", None, None));
        }
        let full_path = resolve_path(&self.working_dir, path);
        if !full_path.exists() {
            return Ok(ToolResponse::error(ToolErrorCode::NotFound.as_str(), &format!("路径 '{}' 不存在", path), None, None));
        }
        if full_path.is_dir() {
            return self.list_directory(path, &full_path);
        }

        let content = fs::read_to_string(&full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("读取文件失败: {}", e)))?;
        let all_lines: Vec<&str> = content.lines().collect();
        let total_lines = all_lines.len();
        let start = offset.min(total_lines);
        let end = (start + limit).min(total_lines);
        let selected_lines = &all_lines[start..end];
        let result_content = selected_lines.join("\n");

        let metadata = fs::metadata(&full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("获取元数据失败: {}", e)))?;
        let mtime = metadata.modified()
            .map(|t| t.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64)
            .unwrap_or(0);
        let file_size_bytes = metadata.len() as i64;

        if let Some(ref registry) = self.registry {
            if let Ok(mut reg) = registry.lock() {
                let mut cache = HashMap::new();
                cache.insert("file_mtime_ms".to_string(), Value::Number(serde_json::Number::from(mtime)));
                cache.insert("file_size_bytes".to_string(), Value::Number(serde_json::Number::from(file_size_bytes)));
                reg.cache_read_metadata(path, cache);
            }
        }

        Ok(ToolResponse::success(
            format!("读取 {} 行（共 {} 行，{} 字节）", selected_lines.len(), total_lines, file_size_bytes),
            Some(serde_json::json!({
                "content": result_content,
                "lines": selected_lines.len(),
                "total_lines": total_lines,
                "file_mtime_ms": mtime,
                "file_size_bytes": file_size_bytes,
                "offset": offset,
                "limit": limit,
            })),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "要读取的文件路径或目录路径（相对项目根目录）", true, None),
            ToolParameter::new("offset", "integer", "起始行号（从 0 开始）", false, Some(Value::Number(0.into()))),
            ToolParameter::new("limit", "integer", "最大行数", false, Some(Value::Number(2000.into()))),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
            project_root: self.project_root.clone(),
            working_dir: self.working_dir.clone(),
            registry: self.registry.clone(),
        })
    }
}

impl ReadTool {
    /// 列出目录内容（兼容 Windows 和 Linux）
    fn list_directory(&self, path: &str, full_path: &Path) -> Result<ToolResponse, HelloAgentException> {
        let mut entries = Vec::new();
        let mut total_files = 0u64;
        let mut total_dirs = 0u64;

        let mut dir_entries: Vec<_> = fs::read_dir(full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("读取目录失败: {}", e)))?
            .filter_map(|e| e.ok())
            .collect();

        dir_entries.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            b_is_dir.cmp(&a_is_dir).then_with(|| a.file_name().cmp(&b.file_name()))
        });

        for entry in dir_entries {
            let file_type = entry.file_type().map_err(|e| HelloAgentException::ToolException(format!("读取类型失败: {}", e)))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = file_type.is_dir();
            let size_str = if is_dir {
                total_dirs += 1;
                "<DIR>".to_string()
            } else {
                total_files += 1;
                match entry.metadata() {
                    Ok(m) => format_size(m.len()),
                    Err(_) => "?".to_string(),
                }
            };
            let mtime_str = match entry.metadata().and_then(|m| m.modified()) {
                Ok(t) => {
                    let ms = t.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
                    format_time(ms)
                }
                Err(_) => "?".to_string(),
            };
            let relative_path = entry.path()
                .strip_prefix(&self.project_root)
                .unwrap_or(&entry.path())
                .to_string_lossy()
                .replace('\\', "/");

            entries.push(serde_json::json!({
                "name": name,
                "type": if is_dir { "directory" } else { "file" },
                "size": size_str,
                "mtime": mtime_str,
                "path": relative_path,
            }));
        }

        let text = if entries.is_empty() {
            format!("目录 '{}' 为空", path)
        } else {
            let mut lines = vec![format!("目录 '{}' 包含 {} 个文件，{} 个目录：\n", path, total_files, total_dirs)];
            for entry in &entries {
                let icon = if entry["type"] == "directory" { "📁" } else { "📄" };
                let name = entry["name"].as_str().unwrap_or("");
                let size = entry["size"].as_str().unwrap_or("");
                let mtime = entry["mtime"].as_str().unwrap_or("");
                lines.push(format!("{} {:<40} {:>10} {}", icon, name, size, mtime));
            }
            lines.join("\n")
        };

        Ok(ToolResponse::success(
            text,
            Some(serde_json::json!({
                "path": path,
                "entries": entries,
                "total_files": total_files,
                "total_dirs": total_dirs,
                "is_directory": true,
            })),
            None,
            None,
        ))
    }
}

// ------------------------------------------------------------
// WriteTool
// ------------------------------------------------------------
/// 文件写入工具
///
///     功能：
///     - 创建或覆盖文件
///     - 乐观锁冲突检测（如果文件已存在）
///     - 原子写入（临时文件 + rename）
///     - 自动备份原文件
///
///     参数：
///     - path: 文件路径
///     - content: 文件内容
///     - file_mtime_ms: 缓存的 mtime（可选，用于冲突检测）
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
        let project_root = project_root.into();
        let default_working = working_dir.clone().unwrap_or_else(|| project_root.clone());
        let working_dir = working_dir
            .unwrap_or(project_root)
            .canonicalize()
            .unwrap_or(default_working);
        Self {
            base: ToolBase::new("Write", "创建或覆盖文件，支持冲突检测和原子写入", false),
            working_dir,
            registry,
        }
    }
}

impl Tool for WriteTool {
    fn name(&self) -> &str { &self.base.name }
    fn base(&self) -> &ToolBase { &self.base }
    fn base_mut(&mut self) -> &mut ToolBase { &mut self.base }

    /// 执行文件写入
    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = parameters.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let cached_mtime = parameters.get("file_mtime_ms").and_then(|v| v.as_i64());

        if path.is_empty() || content.is_empty() {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "缺少必需参数: path 或 content", None, None));
        }
        let full_path = resolve_path(&self.working_dir, path);
        let mut backup_path: Option<PathBuf> = None;

        if full_path.exists() {
            if let Ok(metadata) = fs::metadata(&full_path) {
                if let Ok(mtime) = metadata.modified() {
                    let current_mtime_ms = mtime.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
                    if let Some(cached) = cached_mtime {
                        if current_mtime_ms != cached {
                            return Ok(ToolResponse::error(
                                ToolErrorCode::Conflict.as_str(),
                                &format!("文件自上次读取后被修改。当前 mtime={}, 缓存 mtime={}", current_mtime_ms, cached),
                                None,
                                Some(serde_json::json!({ "current_mtime_ms": current_mtime_ms, "cached_mtime_ms": cached })),
                            ));
                        }
                    }
                }
            }
            backup_path = Some(backup_file(&full_path)
                .map_err(|e| HelloAgentException::ToolException(format!("备份失败: {}", e)))?);
        } else if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| HelloAgentException::ToolException(format!("创建目录失败: {}", e)))?;
        }

        let temp_path = full_path.with_extension("tmp");
        fs::write(&temp_path, content)
            .map_err(|e| HelloAgentException::ToolException(format!("写入临时文件失败: {}", e)))?;
        fs::rename(&temp_path, &full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("原子重命名失败: {}", e)))?;

        let size_bytes = content.len() as u64;
        let backup_rel = backup_path.as_ref().map(|p| {
            p.strip_prefix(&self.working_dir).unwrap_or(p).to_string_lossy().into_owned()
        });

        Ok(ToolResponse::success(
            format!("成功写入 {} ({} 字节)", path, size_bytes),
            Some(serde_json::json!({ "written": true, "size_bytes": size_bytes, "backup_path": backup_rel })),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "文件路径（相对项目根目录）", true, None),
            ToolParameter::new("content", "string", "文件内容", true, None),
            ToolParameter::new("file_mtime_ms", "integer", "缓存的文件修改时间（用于冲突检测）", false, None),
        ]
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
// EditTool
// ------------------------------------------------------------
/// 文件编辑工具
///
///     功能：
///     - 精确替换文件内容（old_string 必须唯一匹配）
///     - 乐观锁冲突检测
///     - 自动备份原文件
///
///     参数：
///     - path: 文件路径
///     - old_string: 要替换的内容
///     - new_string: 替换后的内容
///     - file_mtime_ms: 缓存的 mtime（可选）
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
        let project_root = project_root.into();
        let default_working = working_dir.clone().unwrap_or_else(|| project_root.clone());
        let working_dir = working_dir
            .unwrap_or(project_root)
            .canonicalize()
            .unwrap_or(default_working);
        Self {
            base: ToolBase::new("Edit", "精确替换文件内容，支持冲突检测和自动备份", false),
            working_dir,
            registry,
        }
    }
}

impl Tool for EditTool {
    fn name(&self) -> &str { &self.base.name }
    fn base(&self) -> &ToolBase { &self.base }
    fn base_mut(&mut self) -> &mut ToolBase { &mut self.base }
    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let old = parameters.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
        let new = parameters.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
        let cached_mtime = parameters.get("file_mtime_ms").and_then(|v| v.as_i64());

        if path.is_empty() || old.is_empty() || new.is_empty() {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "缺少必需参数", None, None));
        }
        let full_path = resolve_path(&self.working_dir, path);
        if !full_path.exists() {
            return Ok(ToolResponse::error(ToolErrorCode::NotFound.as_str(), &format!("文件 '{}' 不存在", path), None, None));
        }

        if let Ok(metadata) = fs::metadata(&full_path) {
            if let Ok(mtime) = metadata.modified() {
                let current_ms = mtime.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
                if let Some(cached) = cached_mtime {
                    if current_ms != cached {
                        return Ok(ToolResponse::error(
                            ToolErrorCode::Conflict.as_str(),
                            &format!("文件自上次读取后被修改。当前 mtime={}, 缓存 mtime={}", current_ms, cached),
                            None,
                            Some(serde_json::json!({ "current_mtime_ms": current_ms, "cached_mtime_ms": cached })),
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
            Some(serde_json::json!({ "modified": true, "changed_bytes": changed_bytes, "backup_path": backup_rel })),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "要编辑的文件路径", true, None),
            ToolParameter::new("old_string", "string", "要替换的内容（必须唯一匹配）", true, None),
            ToolParameter::new("new_string", "string", "替换后的内容", true, None),
            ToolParameter::new("file_mtime_ms", "integer", "缓存的文件修改时间", false, None),
        ]
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
// MultiEditTool
// ------------------------------------------------------------
/// 批量编辑工具
///
///     功能：
///     - 批量执行多个替换操作
///     - 原子性保证（要么全部成功，要么全部失败）
///     - 乐观锁冲突检测（所有替换前检查一次）
///
///     参数：
///     - path: 文件路径
///     - edits: 替换列表 [{"old_string": "...", "new_string": "..."}]
///     - file_mtime_ms: 缓存的 mtime（可选）
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
        let project_root = project_root.into();
        let default_working = working_dir.clone().unwrap_or_else(|| project_root.clone());
        let working_dir = working_dir
            .unwrap_or(project_root)
            .canonicalize()
            .unwrap_or(default_working);
        Self {
            base: ToolBase::new("MultiEdit", "批量替换文件内容，支持原子性和冲突检测", false),
            working_dir,
            registry,
        }
    }
}

impl Tool for MultiEditTool {
    fn name(&self) -> &str { &self.base.name }
    fn base(&self) -> &ToolBase { &self.base }
    fn base_mut(&mut self) -> &mut ToolBase { &mut self.base }
    fn run(&self, parameters: Value) -> Result<ToolResponse, HelloAgentException> {
        let path = parameters.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let edits = parameters.get("edits").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let cached_mtime = parameters.get("file_mtime_ms").and_then(|v| v.as_i64());

        if path.is_empty() || edits.is_empty() {
            return Ok(ToolResponse::error(ToolErrorCode::InvalidParam.as_str(), "缺少必需参数", None, None));
        }
        let full_path = resolve_path(&self.working_dir, path);
        if !full_path.exists() {
            return Ok(ToolResponse::error(ToolErrorCode::NotFound.as_str(), &format!("文件 '{}' 不存在", path), None, None));
        }

        if let Ok(metadata) = fs::metadata(&full_path) {
            if let Ok(mtime) = metadata.modified() {
                let current_ms = mtime.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
                if let Some(cached) = cached_mtime {
                    if current_ms != cached {
                        return Ok(ToolResponse::error(
                            ToolErrorCode::Conflict.as_str(),
                            &format!("文件自上次读取后被修改。所有替换已取消。当前 mtime={}, 缓存 mtime={}", current_ms, cached),
                            None,
                            Some(serde_json::json!({ "current_mtime_ms": current_ms, "cached_mtime_ms": cached })),
                        ));
                    }
                }
            }
        }

        let mut content = fs::read_to_string(&full_path)
            .map_err(|e| HelloAgentException::ToolException(format!("读取文件失败: {}", e)))?;
        let original_content = content.clone();

        for (i, edit) in edits.iter().enumerate() {
            let old = edit.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
            let new = edit.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
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
                    &format!("编辑项 {}: old_string 必须唯一匹配。找到 {} 处匹配。", i, matches),
                    Some(serde_json::json!({ "edit_index": i, "matches": matches })),
                    None,
                ));
            }
        }

        for edit in &edits {
            let old = edit.get("old_string").and_then(|v| v.as_str()).unwrap();
            let new = edit.get("new_string").and_then(|v| v.as_str()).unwrap();
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
            format!("成功执行 {} 个替换操作 (变化 {:+} 字节)", edits.len(), changed_bytes),
            Some(serde_json::json!({ "modified": true, "num_edits": edits.len(), "changed_bytes": changed_bytes, "backup_path": backup_rel })),
            None,
            None,
        ))
    }

    fn get_parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter::new("path", "string", "要编辑的文件路径", true, None),
            ToolParameter::new("edits", "array", "替换列表，每项包含 old_string 和 new_string", true, None),
            ToolParameter::new("file_mtime_ms", "integer", "缓存的文件修改时间", false, None),
        ]
    }

    fn box_clone(&self) -> Box<dyn Tool> {
        Box::new(Self {
            base: self.base.clone(),
            working_dir: self.working_dir.clone(),
            registry: self.registry.clone(),
        })
    }
}

// =============================================================================
// 测试用例
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use crate::tools::tool_registry::global_registry;

    fn set_up_registry() -> tokio::sync::MutexGuard<'static, ToolRegistry>{
        tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
            // 全局注册中心
            let guard = global_registry();
            let registry = guard.lock().await;

            registry
        })
    }
    fn setup_temp_dir() -> PathBuf {
        let dir = env::temp_dir().join("hello_agents_file_tests");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_read_file() {
        let dir = setup_temp_dir();
        let file_path = dir.join("test.txt");
        fs::write(&file_path, "line1\nline2\nline3\nline4\n").unwrap();
        let tool = ReadTool::new(dir.clone(), None, None);

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = serde_json::json!({ "path": "test.txt", "offset": 1, "limit": 2 });
        let resp = registry.execute_tool("Read", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================

        // let resp = tool.run(params_json)).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        let data = resp.data;
        assert_eq!(data["content"].as_str().unwrap(), "line2\nline3");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_list_directory() {
        let dir = setup_temp_dir();
        fs::create_dir(dir.join("subdir")).unwrap();
        let tool = ReadTool::new(dir.clone(), None, None);

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = serde_json::json!({ "path": "." });
        let resp = registry.execute_tool("Read", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================
        // 不使用registry直接调用run
        //let resp = tool.run(params_json).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        let data = resp.data;
        assert_eq!(data["is_directory"], true);
        let entries = data["entries"].as_array().unwrap();
        assert!(entries.iter().any(|e| e["name"] == "subdir"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_write_with_conflict() {
        let dir = setup_temp_dir();
        let file_path = dir.join("write_test.txt");
        fs::write(&file_path, "original content").unwrap();
        let tool = WriteTool::new(dir.clone(), None, None);

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = serde_json::json!({
            "path": "write_test.txt",
            "content": "new content",
            "file_mtime_ms": 0,
        });
        let resp = registry.execute_tool("Write", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================
        // 不使用registry直接调用run

        //let resp = tool.run(params_json).unwrap();
        assert_eq!(resp.status, ToolStatus::Error);
        assert_eq!(resp.error_info.as_ref().unwrap().code, ToolErrorCode::Conflict.as_str());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_edit() {
        let dir = setup_temp_dir();
        let file_path = dir.join("edit_test.txt");
        fs::write(&file_path, "Hello world").unwrap();
        let tool = EditTool::new(dir.clone(), None, None);

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = serde_json::json!({
            "path": "edit_test.txt",
            "old_string": "world",
            "new_string": "Rust",
        });
        let resp = registry.execute_tool("Edit", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================
        // 不使用registry直接调用run
        //let resp = tool.run(params_json).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "Hello Rust");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_multi_edit() {
        let dir = setup_temp_dir();
        let file_path = dir.join("multi.txt");
        fs::write(&file_path, "A B C").unwrap();
        let tool = MultiEditTool::new(dir.clone(), None, None);

        // ================= 采用注册中心处理 start======================
        let mut registry = set_up_registry();
        registry.register_tool(Box::new(tool), true);
        let params_json = serde_json::json!({
            "path": "multi.txt",
            "edits": [
                {"old_string": "A", "new_string": "1"},
                {"old_string": "B", "new_string": "2"},
            ],
        });
        let resp = registry.execute_tool("MultiEdit", serde_json::to_string(&params_json).unwrap().as_str());
        println!("{:?}", resp);
        // ================= 采用注册中心处理 end======================
        // 不使用registry直接调用run
        // let resp = tool.run(params_json).unwrap();
        assert_eq!(resp.status, ToolStatus::Success);
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "1 2 C");
        fs::remove_dir_all(&dir).ok();
    }
}