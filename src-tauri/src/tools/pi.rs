use super::{
    RenameKind, SessionRow, ToolId, file_mtime_millis, first_string, matches_cwd, truncate_title,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn pi_home() -> PathBuf {
    std::env::var_os("PI_CODING_AGENT_DIR")
        .or_else(|| std::env::var_os("PI_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".pi")
        })
}

pub fn sessions_root() -> PathBuf {
    pi_home().join("agent").join("sessions")
}

pub fn encode_cwd(cwd: &str) -> String {
    let mut text = cwd.trim().trim_matches('"').replace('\\', "/");
    while text.len() > 1 && text.ends_with('/') {
        text.pop();
    }
    let trimmed = text.trim_start_matches('/');
    format!("--{}--", trimmed.replace(['/', ':'], "-"))
}

pub fn list_sessions(cwd: &str) -> Result<Vec<SessionRow>, String> {
    list_sessions_in(&sessions_root(), cwd)
}

pub fn list_sessions_in(root: &Path, cwd: &str) -> Result<Vec<SessionRow>, String> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    let preferred = root.join(encode_cwd(cwd));
    let mut dirs = Vec::new();
    if preferred.is_dir() {
        dirs.push(preferred);
    }
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && !dirs.iter().any(|item| item == &path) {
                dirs.push(path);
            }
        }
    }
    for dir in dirs {
        let Ok(files) = fs::read_dir(&dir) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            let parsed = parse_session_file(&path, cwd);
            let Some((id, title, session_cwd, updated_at)) = parsed else {
                continue;
            };
            if !matches_cwd(&session_cwd, cwd) && dir.file_name().and_then(|n| n.to_str()) != Some(&encode_cwd(cwd))
            {
                continue;
            }
            if !seen.insert(id.clone()) {
                continue;
            }
            rows.push(SessionRow {
                tool_id: ToolId::Pi,
                id,
                title,
                cwd: cwd.to_string(),
                updated_at,
                rename_kind: RenameKind::Overlay,
            });
        }
    }
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(rows)
}

pub fn delete_session(cwd: &str, session_id: &str) -> Result<(), String> {
    let path = find_session_file(&sessions_root(), cwd, session_id).ok_or_else(|| {
        "没有找到这个 Pi 会话文件，无法删除。请确认 ~/.pi/agent/sessions 还在。".to_string()
    })?;
    fs::remove_file(&path).map_err(|err| format!("删除 Pi 会话失败：{err}"))
}

fn find_session_file(root: &Path, cwd: &str, session_id: &str) -> Option<PathBuf> {
    let rows = list_sessions_in(root, cwd).ok()?;
    if !rows.iter().any(|row| row.id == session_id) {
        return None;
    }
    let preferred = root.join(encode_cwd(cwd));
    let mut dirs = Vec::new();
    if preferred.is_dir() {
        dirs.push(preferred);
    }
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            }
        }
    }
    for dir in dirs {
        let Ok(files) = fs::read_dir(&dir) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            if let Some((id, _, _, _)) = parse_session_file(&path, cwd) {
                if id == session_id {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn parse_session_file(path: &Path, fallback_cwd: &str) -> Option<(String, String, String, i64)> {
    let text = fs::read_to_string(path).ok()?;
    let mut id = None;
    let mut name = None;
    let mut first_user = None;
    let mut session_cwd = fallback_cwd.to_string();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match value.get("type").and_then(|item| item.as_str()) {
            Some("session") => {
                if let Some(found) = value.get("id").and_then(|item| item.as_str()) {
                    if !found.trim().is_empty() {
                        id = Some(found.trim().to_string());
                    }
                }
                if let Some(found) = value.get("cwd").and_then(|item| item.as_str()) {
                    if !found.trim().is_empty() {
                        session_cwd = found.trim().to_string();
                    }
                }
            }
            Some("session_info") => {
                if let Some(found) = value.get("name").and_then(|item| item.as_str()) {
                    let trimmed = found.trim();
                    if !trimmed.is_empty() {
                        name = Some(trimmed.to_string());
                    }
                }
            }
            Some("message") if first_user.is_none() => {
                if value
                    .get("message")
                    .and_then(|item| item.get("role"))
                    .and_then(|item| item.as_str())
                    == Some("user")
                {
                    first_user = user_text(value.get("message").unwrap_or(&Value::Null));
                }
            }
            _ => {}
        }
    }
    let id = id.or_else(|| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.rsplit_once('_').map(|(_, id)| id.to_string()))
            .filter(|id| !id.is_empty())
    })?;
    let title = name
        .or(first_user)
        .map(|text| truncate_title(&text, 48))
        .unwrap_or_else(|| "Pi 会话".into());
    Some((id, title, session_cwd, file_mtime_millis(path)))
}

fn user_text(value: &Value) -> Option<String> {
    if let Some(s) = first_string(value, &["text", "content"]) {
        return Some(s);
    }
    let content = value.get("content")?;
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
    fn encodes_cwd_like_official_pi() {
        assert_eq!(encode_cwd("/Users/me/work/app"), "--Users-me-work-app--");
        assert_eq!(
            encode_cwd(r"D:\idea_jidian_projects\agent-dock"),
            "--D--idea_jidian_projects-agent-dock--"
        );
    }

    #[test]
    fn lists_named_jsonl_session() {
        let root = std::env::temp_dir().join(format!("ad-pi-{}", std::process::id()));
        let cwd = "/tmp/pi-demo";
        let dir = root.join(encode_cwd(cwd));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("20241203T140000_sess-pi-1.jsonl");
        let mut out = fs::File::create(&file).unwrap();
        writeln!(
            out,
            r#"{{"type":"session","version":3,"id":"sess-pi-1","cwd":"/tmp/pi-demo"}}"#
        )
        .unwrap();
        writeln!(
            out,
            r#"{{"type":"session_info","name":"Refactor auth"}}"#
        )
        .unwrap();
        writeln!(
            out,
            r#"{{"type":"message","message":{{"role":"user","content":"hello"}}}}"#
        )
        .unwrap();
        let rows = list_sessions_in(&root, cwd).unwrap();
        fs::remove_dir_all(&root).ok();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "sess-pi-1");
        assert_eq!(rows[0].title, "Refactor auth");
        assert_eq!(rows[0].tool_id, ToolId::Pi);
    }
}
