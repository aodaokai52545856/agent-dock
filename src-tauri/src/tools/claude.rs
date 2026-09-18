use super::{
    RenameKind, SessionRow, ToolId, file_mtime_millis, first_string, matches_cwd, parse_time_value,
    truncate_title,
};
use crate::path_norm::normalize_path;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const REGISTRY_RECENT_MS: i64 = 48 * 60 * 60 * 1000;

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

pub fn find_session_file(cwd: &str, session_id: &str) -> Option<PathBuf> {
    find_session_file_in(&claude_home(), cwd, session_id)
}

pub fn find_session_file_in(home: &Path, cwd: &str, session_id: &str) -> Option<PathBuf> {
    let id = session_id.trim();
    if id.is_empty() {
        return None;
    }
    for name in project_dir_names(cwd) {
        let path = home.join("projects").join(name).join(format!("{id}.jsonl"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn list_sessions(cwd: &str) -> Result<Vec<SessionRow>, String> {
    list_sessions_in(&claude_home(), cwd)
}

pub fn list_sessions_in(home: &Path, cwd: &str) -> Result<Vec<SessionRow>, String> {
    let mut rows = BTreeMap::new();
    let mut metas = BTreeMap::new();
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
            if stem.is_empty() || stem.starts_with("agent-") || rows.contains_key(stem) {
                continue;
            }
            let meta = jsonl_meta(&path);
            rows.insert(
                stem.to_string(),
                SessionRow {
                    tool_id: ToolId::Claude,
                    id: stem.to_string(),
                    title: pick_title(&meta, None, None),
                    cwd: cwd.to_string(),
                    updated_at: file_mtime_millis(&path),
                    rename_kind: RenameKind::Native,
                },
            );
            metas.insert(stem.to_string(), meta);
        }
    }
    merge_registry(home, cwd, &mut rows, &metas);
    let mut out: Vec<SessionRow> = rows.into_values().collect();
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(out)
}

pub fn rename_session(cwd: &str, session_id: &str, title: &str) -> Result<(), String> {
    rename_session_in(&claude_home(), cwd, session_id, title)
}

pub fn rename_session_in(
    home: &Path,
    cwd: &str,
    session_id: &str,
    title: &str,
) -> Result<(), String> {
    let mut wrote = false;
    if let Some(path) = jsonl_file(home, cwd, session_id) {
        append_custom_title(&path, session_id, title)?;
        wrote = true;
    }
    if update_registry_name(home, session_id, title) {
        wrote = true;
    }
    if !wrote {
        return Err(
            "没有找到这个 Claude 会话文件，无法改名。请确认 ~/.claude/projects 还在。".into(),
        );
    }
    Ok(())
}

pub fn delete_session(session_id: &str, cwd: &str) -> Result<(), String> {
    let home = claude_home();
    let jsonl = jsonl_file(&home, cwd, session_id);
    let registry = registry_files(&home, session_id);
    if jsonl.is_none() && registry.is_empty() {
        return Err("没有找到这个 Claude 会话文件，无法删除。请确认 ~/.claude/projects 还在。".into());
    }
    if let Some(path) = jsonl {
        fs::remove_file(&path).map_err(|err| format!("删除 Claude 会话失败：{err}"))?;
    }
    for path in registry {
        let _ = fs::remove_file(&path);
    }
    Ok(())
}

fn merge_registry(
    home: &Path,
    cwd: &str,
    rows: &mut BTreeMap<String, SessionRow>,
    metas: &BTreeMap<String, JsonlMeta>,
) {
    let dir = home.join("sessions");
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    let now = now_millis();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(reg) = parse_registry(&path) else {
            continue;
        };
        if reg.cwd.is_empty() || !matches_cwd(&reg.cwd, cwd) {
            continue;
        }
        if let Some(existing) = rows.get_mut(&reg.id) {
            let meta = metas.get(&reg.id).cloned().unwrap_or_default();
            existing.title = pick_title(&meta, reg.name.as_deref(), reg.name_source.as_deref());
            existing.updated_at = existing.updated_at.max(reg.updated_at);
            continue;
        }
        if now.saturating_sub(file_mtime_millis(&path)) > REGISTRY_RECENT_MS {
            continue;
        }
        rows.insert(
            reg.id.clone(),
            SessionRow {
                tool_id: ToolId::Claude,
                id: reg.id,
                title: pick_title(
                    &JsonlMeta::default(),
                    reg.name.as_deref(),
                    reg.name_source.as_deref(),
                ),
                cwd: cwd.to_string(),
                updated_at: reg.updated_at,
                rename_kind: RenameKind::Native,
            },
        );
    }
}

fn jsonl_file(home: &Path, cwd: &str, session_id: &str) -> Option<PathBuf> {
    for name in project_dir_names(cwd) {
        let path = home
            .join("projects")
            .join(name)
            .join(format!("{session_id}.jsonl"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn registry_files(home: &Path, session_id: &str) -> Vec<PathBuf> {
    let dir = home.join("sessions");
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().and_then(|e| e.to_str()) == Some("json")
                && parse_registry(path).is_some_and(|row| row.id == session_id)
        })
        .collect()
}

fn append_custom_title(path: &Path, session_id: &str, title: &str) -> Result<(), String> {
    let line = json!({
        "type": "custom-title",
        "customTitle": title,
        "sessionId": session_id,
    });
    let mut data = fs::read(path).unwrap_or_default();
    if !data.is_empty() && !data.ends_with(b"\n") {
        data.push(b'\n');
    }
    data.extend_from_slice(line.to_string().as_bytes());
    data.push(b'\n');
    let tmp = path.with_extension("jsonl.tmp");
    fs::write(&tmp, &data).map_err(|err| format!("写入标题失败：{err}"))?;
    fs::rename(&tmp, path).map_err(|err| format!("保存标题失败：{err}"))?;
    Ok(())
}

fn update_registry_name(home: &Path, session_id: &str, title: &str) -> bool {
    let mut wrote = false;
    for path in registry_files(home, session_id) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(obj) = value.as_object_mut() else {
            continue;
        };
        obj.insert("name".into(), json!(title));
        obj.insert("nameSource".into(), json!("user"));
        obj.insert("nameSince".into(), json!(now_millis()));
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, value.to_string()).is_ok() && fs::rename(&tmp, &path).is_ok() {
            wrote = true;
        }
    }
    wrote
}

#[derive(Clone, Default)]
struct JsonlMeta {
    custom_title: Option<String>,
    ai_title: Option<String>,
    user_text: Option<String>,
}

fn jsonl_meta(path: &Path) -> JsonlMeta {
    let mut meta = JsonlMeta::default();
    let text = fs::read_to_string(path).unwrap_or_default();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match value.get("type").and_then(|v| v.as_str()) {
            Some("custom-title") => {
                if let Some(title) = first_string(&value, &["customTitle", "title"]) {
                    meta.custom_title = Some(title);
                }
            }
            Some("ai-title") => {
                if let Some(title) = first_string(&value, &["aiTitle", "title"]) {
                    meta.ai_title = Some(title);
                }
            }
            Some("user") => {
                if meta.user_text.is_none() {
                    meta.user_text = user_text(&value);
                }
            }
            _ => {}
        }
    }
    meta
}

fn pick_title(meta: &JsonlMeta, registry_name: Option<&str>, name_source: Option<&str>) -> String {
    if let Some(title) = &meta.custom_title {
        return truncate_title(title, 48);
    }
    let named_by_user = name_source
        .map(|source| {
            let source = source.to_ascii_lowercase();
            source != "derived" && source != "auto"
        })
        .unwrap_or(false);
    if named_by_user {
        if let Some(name) = registry_name.filter(|value| !value.is_empty()) {
            return truncate_title(name, 48);
        }
    }
    if let Some(title) = &meta.ai_title {
        return truncate_title(title, 48);
    }
    if let Some(title) = &meta.user_text {
        return truncate_title(title, 48);
    }
    if let Some(name) = registry_name.filter(|value| !value.is_empty()) {
        return truncate_title(name, 48);
    }
    "Claude 会话".into()
}

struct RegistryRow {
    id: String,
    cwd: String,
    name: Option<String>,
    name_source: Option<String>,
    updated_at: i64,
}

fn parse_registry(path: &Path) -> Option<RegistryRow> {
    let text = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let id = first_string(&value, &["sessionId", "session_id"])?;
    if id.is_empty() {
        return None;
    }
    let cwd = first_string(&value, &["cwd", "workDir"]).unwrap_or_default();
    let name = first_string(&value, &["name", "title"]);
    let name_source = first_string(&value, &["nameSource", "name_source"]);
    let updated_at = parse_time_value(value.get("updatedAt").unwrap_or(&Value::Null))
        .or_else(|| parse_time_value(value.get("startedAt").unwrap_or(&Value::Null)))
        .unwrap_or_else(|| file_mtime_millis(path));
    Some(RegistryRow {
        id,
        cwd,
        name,
        name_source,
        updated_at,
    })
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
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
        let found = find_session_file_in(&root, cwd, "abc-123");
        fs::remove_dir_all(&root).ok();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "abc-123");
        assert_eq!(rows[0].tool_id, ToolId::Claude);
        assert!(rows[0].title.contains("Fix the login form"));
        assert_eq!(found.as_deref(), Some(dir.join("abc-123.jsonl").as_path()));
    }

    #[test]
    fn lists_live_registry_session_before_jsonl_exists() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = r"D:\idea_jidian_projects\agent-dock";
        fs::create_dir_all(dir.path().join("sessions")).unwrap();
        fs::write(
            dir.path().join("sessions").join("66264.json"),
            r#"{"pid":66264,"sessionId":"0cec7285-1d97-43d3-be68-35173ce07ce1","cwd":"D:\\idea_jidian_projects\\agent-dock","name":"login-fix","nameSource":"user","startedAt":1789269198631,"updatedAt":1789269198734}"#,
        )
        .unwrap();
        fs::write(dir.path().join("sessions").join("66264.abc.key"), "nope").unwrap();
        let rows = list_sessions_in(dir.path(), cwd).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "0cec7285-1d97-43d3-be68-35173ce07ce1");
        assert_eq!(rows[0].title, "login-fix");
        assert_eq!(rows[0].tool_id, ToolId::Claude);
    }

    #[test]
    fn skips_registry_session_for_other_cwd() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("sessions")).unwrap();
        fs::write(
            dir.path().join("sessions").join("1.json"),
            r#"{"sessionId":"other","cwd":"D:\\other-project","name":"nope","updatedAt":1789269198734}"#,
        )
        .unwrap();
        let rows = list_sessions_in(dir.path(), r"D:\idea_jidian_projects\agent-dock").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn prefers_custom_title_over_first_user_message() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = "/tmp/demo-app";
        let project = dir.path().join("projects").join(encode_project_dir(cwd));
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join("abc-123.jsonl"),
            concat!(
                r#"{"type":"user","message":{"role":"user","content":[{"type":"text","text":"Fix the login form"}]}}"#,
                "\n",
                r#"{"type":"custom-title","customTitle":"auth-rewrite","sessionId":"abc-123"}"#,
                "\n"
            ),
        )
        .unwrap();
        let rows = list_sessions_in(dir.path(), cwd).unwrap();
        assert_eq!(rows[0].title, "auth-rewrite");
    }

    #[test]
    fn rename_writes_custom_title_and_registry_name() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = "/tmp/demo-app";
        let project = dir.path().join("projects").join(encode_project_dir(cwd));
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(dir.path().join("sessions")).unwrap();
        fs::write(project.join("abc-123.jsonl"), "{\"type\":\"mode\"}\n").unwrap();
        fs::write(
            dir.path().join("sessions").join("99.json"),
            r#"{"sessionId":"abc-123","cwd":"/tmp/demo-app","name":"old","nameSource":"derived"}"#,
        )
        .unwrap();
        rename_session_in(dir.path(), cwd, "abc-123", "登录超时").unwrap();
        let jsonl = fs::read_to_string(project.join("abc-123.jsonl")).unwrap();
        assert!(jsonl.contains(r#""type":"custom-title""#));
        assert!(jsonl.contains("登录超时"));
        let registry: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.path().join("sessions").join("99.json")).unwrap())
                .unwrap();
        assert_eq!(registry["name"], "登录超时");
        let rows = list_sessions_in(dir.path(), cwd).unwrap();
        assert_eq!(rows[0].title, "登录超时");
    }
}
