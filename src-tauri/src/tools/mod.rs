mod grokbuild;
mod kimi;
mod opencode;
pub mod update;

use crate::path_norm::normalize_path;
use crate::platform;
use crate::state::AppSettings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ToolId {
    Opencode,
    Grokbuild,
    Kimi,
}

impl ToolId {
    pub fn as_str(self) -> &'static str {
        match self {
            ToolId::Opencode => "opencode",
            ToolId::Grokbuild => "grokbuild",
            ToolId::Kimi => "kimi",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            ToolId::Opencode => "OpenCode",
            ToolId::Grokbuild => "Grok Build",
            ToolId::Kimi => "Kimi",
        }
    }

    fn override_path(self, settings: &AppSettings) -> &str {
        match self {
            ToolId::Opencode => &settings.opencode_path,
            ToolId::Grokbuild => &settings.grokbuild_path,
            ToolId::Kimi => &settings.kimi_path,
        }
    }

    fn candidates(self) -> &'static [&'static str] {
        match self {
            ToolId::Opencode => &["opencode"],
            ToolId::Grokbuild => &["grokbuild", "grok"],
            ToolId::Kimi => &["kimi"],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub tool_id: ToolId,
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub updated_at: i64,
    pub rename_kind: RenameKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RenameKind {
    Native,
    Overlay,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryProbe {
    pub found: bool,
    pub path: Option<String>,
    pub message: Option<String>,
}

pub fn resolve_binary(tool: ToolId, settings: &AppSettings) -> Result<PathBuf, String> {
    let override_path = tool.override_path(settings).trim();
    if !override_path.is_empty() {
        let path = PathBuf::from(override_path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "设置里的 {} 路径不存在：{}。请改成实际可执行文件，或清空后使用 PATH。",
            tool.display_name(),
            override_path
        ));
    }
    for name in tool.candidates() {
        if let Some(path) = platform::which_cmd(name) {
            return Ok(path);
        }
    }
    Err(format!(
        "没有找到 {} 命令（{}）。请把它加进 PATH，或在设置里填写可执行文件路径。",
        tool.display_name(),
        tool.candidates().join(" / ")
    ))
}

pub fn probe_binary(tool: ToolId, settings: &AppSettings) -> BinaryProbe {
    match resolve_binary(tool, settings) {
        Ok(path) => BinaryProbe {
            found: true,
            path: Some(path.display().to_string()),
            message: None,
        },
        Err(message) => BinaryProbe {
            found: false,
            path: None,
            message: Some(message),
        },
    }
}

pub fn list_sessions(tool: ToolId, cwd: &str, settings: &AppSettings) -> Result<Vec<SessionRow>, String> {
    match tool {
        ToolId::Opencode => opencode::list_sessions(cwd, settings),
        ToolId::Grokbuild => grokbuild::list_sessions(cwd),
        ToolId::Kimi => kimi::list_sessions(cwd),
    }
}

pub fn rename_session(
    tool: ToolId,
    cwd: &str,
    session_id: &str,
    title: &str,
    _settings: &AppSettings,
) -> Result<RenameKind, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("名称不能为空".into());
    }
    if title.chars().count() > 80 {
        return Err("名称请控制在 80 个字以内".into());
    }
    match tool {
        ToolId::Opencode => Ok(RenameKind::Overlay),
        ToolId::Grokbuild => grokbuild::rename_session(cwd, session_id, title).map(|_| RenameKind::Native),
        ToolId::Kimi => kimi::rename_session(session_id, title).map(|_| RenameKind::Native),
    }
}

pub fn delete_session(
    tool: ToolId,
    cwd: &str,
    session_id: &str,
    settings: &AppSettings,
) -> Result<(), String> {
    match tool {
        ToolId::Opencode => opencode::delete_session(cwd, session_id, settings),
        ToolId::Grokbuild => grokbuild::delete_session(cwd, session_id),
        ToolId::Kimi => kimi::delete_session(session_id),
    }
}

pub fn resume_args(tool: ToolId, session_id: &str) -> Vec<String> {
    match tool {
        ToolId::Opencode => vec!["--session".into(), session_id.into()],
        ToolId::Grokbuild => vec!["--resume".into(), session_id.into()],
        ToolId::Kimi => vec!["--session".into(), session_id.into()],
    }
}

pub fn launch_args(tool: ToolId, session_id: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if tool == ToolId::Grokbuild {
        args.push("--fullscreen".into());
    }
    if let Some(id) = session_id {
        args.extend(resume_args(tool, id));
    }
    args
}

pub fn truncate_title(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim().replace('\n', " ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub fn parse_time_value(value: &serde_json::Value) -> Option<i64> {
    if let Some(n) = value.as_i64() {
        return Some(normalize_epoch(n));
    }
    if let Some(n) = value.as_u64() {
        return Some(normalize_epoch(n as i64));
    }
    if let Some(n) = value.as_f64() {
        return Some(normalize_epoch(n as i64));
    }
    if let Some(s) = value.as_str() {
        if let Ok(n) = s.parse::<i64>() {
            return Some(normalize_epoch(n));
        }
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
            return Some(dt.timestamp_millis());
        }
    }
    None
}

fn normalize_epoch(n: i64) -> i64 {
    if n > 1_000_000_000_000 {
        n
    } else if n > 1_000_000_000 {
        n * 1000
    } else {
        n
    }
}

pub fn file_mtime_millis(path: &Path) -> i64 {
    path.metadata()
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|dur| dur.as_millis() as i64)
        .unwrap_or(0)
}

pub fn first_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = value.get(*key).and_then(|v| v.as_str()) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

pub fn matches_cwd(session_cwd: &str, project_cwd: &str) -> bool {
    normalize_path(session_cwd) == normalize_path(project_cwd)
}

pub fn grok_home() -> PathBuf {
    grokbuild::grok_home()
}

pub fn kimi_home() -> PathBuf {
    kimi::kimi_home()
}

pub fn find_grok_session_dir(cwd: &str, session_id: &str) -> Option<PathBuf> {
    grokbuild::find_session_dir(cwd, session_id)
}

pub fn find_kimi_session_dir(session_id: &str) -> Option<PathBuf> {
    kimi::find_session_dir(session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_argv_matches_official_flags() {
        assert_eq!(resume_args(ToolId::Opencode, "ses_1"), vec!["--session", "ses_1"]);
        assert_eq!(resume_args(ToolId::Grokbuild, "abc"), vec!["--resume", "abc"]);
        assert_eq!(resume_args(ToolId::Kimi, "session_1"), vec!["--session", "session_1"]);
    }

    #[test]
    fn grok_launch_forces_fullscreen_so_jetbrains_env_cannot_auto_minimal() {
        assert_eq!(launch_args(ToolId::Grokbuild, None), vec!["--fullscreen"]);
        assert_eq!(
            launch_args(ToolId::Grokbuild, Some("abc")),
            vec!["--fullscreen", "--resume", "abc"]
        );
        assert!(launch_args(ToolId::Opencode, None).is_empty());
        assert_eq!(
            launch_args(ToolId::Kimi, Some("session_1")),
            vec!["--session", "session_1"]
        );
    }

    #[test]
    fn truncate_keeps_short_titles() {
        assert_eq!(truncate_title("hello", 20), "hello");
        assert_eq!(truncate_title("abcdefghij", 6).chars().count(), 6);
    }
}
