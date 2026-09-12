use super::{
    SessionRow, ToolId, file_mtime_millis, first_string, truncate_title,
};
use crate::path_norm::normalize_path;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub fn claude_home() -> PathBuf {
    std::env::var_os("CLAUDE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".claude"))
}

pub fn encode_project_dir(cwd: &str) -> String {
    cwd.trim()
        .trim_matches('"')
        .replace('\\', "/")
        .trim_end_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn project_dir_names(cwd: &str) -> Vec<String> {
    let mut names = vec![encode_project_dir(cwd), encode_project_dir(&normalize_path(cwd))];
    names.sort();
    names.dedup();
    names
}

pub fn list_sessions(cwd: &str) -> Result<Vec<SessionRow>, String> {
    list_sessions_in(&claude_home(), cwd)
}

pub fn list_sessions_in(home: &Path, cwd: &str) -> Result<Vec<SessionRow>, String> {
    let mut rows = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for name in project_dir_names(cwd) {
        let dir = home.join("projects").join(name);
        if !dir.is_dir() {
            continue;
        }
        let entries = fs::read_dir(&dir).map_err(|err| format!("无法读取 Claude session 目录：{err}"))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.is_empty() || stem.starts_with("agent-") || !seen.insert(stem.to_string()) {
                continue;
            }
            rows.push(SessionRow {
                tool_id: ToolId::Claude,
                id: stem.to_string(),
                title: title_from_jsonl(&path),
                cwd: cwd.to_string(),
                updated_at: file_mtime_millis(&path),
                rename_kind: super::RenameKind::Overlay,
            });
        }
    }
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(rows)
}

pub fn delete_session(session_id: &str, cwd: &str) -> Result<(), String> {
    let path = session_file(&claude_home(), cwd, session_id).ok_or_else(|| {
        "没有找到这个 Claude 会话文件，无法删除。请确认 ~/.claude/projects 还在。".to_string()
    })?;
    fs::remove_file(&path).map_err(|err| format!("删除 Claude 会话失败：{err}"))
}

fn session_file(home: &Path, cwd: &str, session_id: &str) -> Option<PathBuf> {
    let path = home
        .join("projects")
        .join(encode_project_dir(cwd))
        .join(format!("{session_id}.jsonl"));
    path.is_file().then_some(path)
}

fn title_from_jsonl(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("type").and_then(|v| v.as_str()) != Some("user") {
            continue;
        }
        if let Some(title) = user_text(&value) {
            return truncate_title(&title, 48);
        }
    }
    "Claude 会话".into()
}

fn user_text(value: &Value) -> Option<String> {
    if let Some(s) = first_string(value, &["text", "content"]) {
        return Some(s);
    }
    let message = value.get("message")?;
    if let Some(s) = message.as_str() {
        let t = s.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Some(s) = first_string(message, &["content", "text"]) {
        return Some(s);
    }
    let content = message.get("content")?;
    if let Some(s) = content.as_str() {
        let t = s.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Some(arr) = content.as_array() {
        for item in arr {
            if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                if let Some(s) = item.get("text").and_then(|v| v.as_str()) {
                    let t = s.trim();
                    if !t.is_empty() {
                        return Some(t.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn encodes_windows_and_unix_cwds() {
        assert_eq!(
            encode_project_dir(r"D:\idea_jidian_projects\agent-dock"),
            "D--idea-jidian-projects-agent-dock"
        );
        assert_eq!(
            encode_project_dir("/Users/me/work/app"),
            "-Users-me-work-app"
        );
    }

    #[test]
    fn lists_jsonl_sessions_for_matching_project() {
        let root = std::env::temp_dir().join(format!("ad-claude-{}", std::process::id()));
        let cwd = "/tmp/demo-app";
        let dir = root.join("projects").join(encode_project_dir(cwd));
        fs::create_dir_all(&dir).unwrap();
        let mut file = fs::File::create(dir.join("abc-123.jsonl")).unwrap();
        writeln!(
            file,
            r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"Fix the login form"}}]}}}}"#
        )
        .unwrap();
        fs::write(dir.join("agent-skip.jsonl"), "{}\n").unwrap();
        let rows = list_sessions_in(&root, cwd).unwrap();
        fs::remove_dir_all(&root).ok();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "abc-123");
        assert_eq!(rows[0].tool_id, ToolId::Claude);
        assert!(rows[0].title.contains("Fix the login form"));
    }
}
