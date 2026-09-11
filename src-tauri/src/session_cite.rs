use crate::path_norm::normalize_path;
use crate::state::{self, Project};
use crate::tools::{self, ToolId, file_mtime_millis, first_string, truncate_title};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

const JSONL_BYTE_CAP: u64 = 8 * 1024 * 1024;
const DOC_BYTE_CAP: u64 = 1_500_000;
const MAX_DOCS: usize = 80;
const MAX_TURNS: usize = 80;
const MAX_TURN_CHARS: usize = 20_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionDoc {
    pub kind: String,
    pub title: String,
    pub path: String,
    pub rel_path: Option<String>,
    pub updated_at: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionDocBody {
    pub path: String,
    pub title: String,
    pub text: String,
    pub rel_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionTurn {
    pub id: String,
    pub role: String,
    pub excerpt: String,
    pub text: String,
}

pub fn list_session_docs(
    app: &AppHandle,
    project_id: &str,
    tool_id: ToolId,
    session_id: &str,
) -> Result<Vec<SessionDoc>, String> {
    let state = state::load_state(app)?;
    let project = state::find_project(&state, project_id)?;
    list_session_docs_for(project, tool_id, session_id)
}

pub fn read_session_doc(app: &AppHandle, project_id: &str, path: &str) -> Result<SessionDocBody, String> {
    let state = state::load_state(app)?;
    let project = state::find_project(&state, project_id)?;
    read_session_doc_for(project, path)
}

pub fn list_session_turns(
    app: &AppHandle,
    project_id: &str,
    tool_id: ToolId,
    session_id: &str,
) -> Result<Vec<SessionTurn>, String> {
    let state = state::load_state(app)?;
    let project = state::find_project(&state, project_id)?;
    list_session_turns_for(project, tool_id, session_id)
}

fn list_session_docs_for(
    project: &Project,
    tool_id: ToolId,
    session_id: &str,
) -> Result<Vec<SessionDoc>, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err("请先打开会话".into());
    }
    match tool_id {
        ToolId::Opencode => Ok(Vec::new()),
        ToolId::Grokbuild => {
            let dir = tools::find_grok_session_dir(&project.path, session_id).ok_or_else(|| {
                "没有找到这个 Grok 会话的本地目录，无法列出文档。".to_string()
            })?;
            Ok(collect_grok_docs(&dir, session_id, &project.path))
        }
        ToolId::Kimi => {
            let dir = tools::find_kimi_session_dir(session_id)
                .ok_or_else(|| "没有找到这个 Kimi 会话的本地目录，无法列出文档。".to_string())?;
            Ok(collect_kimi_docs(&dir, &project.path))
        }
    }
}

fn list_session_turns_for(
    project: &Project,
    tool_id: ToolId,
    session_id: &str,
) -> Result<Vec<SessionTurn>, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err("请先打开会话".into());
    }
    match tool_id {
        ToolId::Opencode => Ok(Vec::new()),
        ToolId::Grokbuild => {
            let dir = tools::find_grok_session_dir(&project.path, session_id).ok_or_else(|| {
                "没有找到这个 Grok 会话的本地目录，无法读取对话。".to_string()
            })?;
            let text = read_text_capped(&dir.join("chat_history.jsonl"), JSONL_BYTE_CAP).unwrap_or_default();
            Ok(parse_grok_turns(&text))
        }
        ToolId::Kimi => {
            let dir = tools::find_kimi_session_dir(session_id)
                .ok_or_else(|| "没有找到这个 Kimi 会话的本地目录，无法读取对话。".to_string())?;
            let wire = dir.join("agents").join("main").join("wire.jsonl");
            let text = read_text_capped(&wire, JSONL_BYTE_CAP).unwrap_or_default();
            Ok(parse_kimi_turns(&text))
        }
    }
}

fn read_session_doc_for(project: &Project, path: &str) -> Result<SessionDocBody, String> {
    let raw = PathBuf::from(path.trim());
    if path.trim().is_empty() {
        return Err("文档路径无效".into());
    }
    if !raw.is_file() {
        return Err("文档不存在或已被删掉".into());
    }
    if !path_allowed(&raw, project) {
        return Err("只能读取当前项目或本机会话目录里的文档".into());
    }
    let meta = fs::metadata(&raw).map_err(|err| format!("读取文档失败：{err}"))?;
    if meta.len() > DOC_BYTE_CAP {
        return Err("文档太大，请在编辑器里打开".into());
    }
    let text = fs::read_to_string(&raw).map_err(|err| format!("读取文档失败：{err}"))?;
    Ok(SessionDocBody {
        title: doc_title(&raw),
        rel_path: rel_project_path(&raw, &project.path),
        path: raw.to_string_lossy().into_owned(),
        text,
    })
}

fn collect_grok_docs(session_dir: &Path, session_id: &str, project_cwd: &str) -> Vec<SessionDoc> {
    let mut docs: HashMap<String, SessionDoc> = HashMap::new();
    push_doc(&mut docs, session_dir.join("plan.md"), project_cwd, "grokbuild");
    // Prefer hunk_records over updates.jsonl. The latter is a full event stream
    // (often 8–17MB) and must not be parsed on the 15s rail refresh.
    if let Some(text) = read_text_capped(&session_dir.join("hunk_records.jsonl"), JSONL_BYTE_CAP) {
        collect_hunk_md(&text, session_id, project_cwd, session_dir, &mut docs);
    }
    finish_docs(docs)
}

fn collect_kimi_docs(session_dir: &Path, project_cwd: &str) -> Vec<SessionDoc> {
    let mut docs: HashMap<String, SessionDoc> = HashMap::new();
    let plans = session_dir.join("agents").join("main").join("plans");
    collect_md_dir(&plans, project_cwd, "kimi", &mut docs);
    let versions = session_dir.join("agents").join("main").join("plan");
    collect_md_tree(&versions, project_cwd, "kimi", &mut docs);
    let wire = session_dir.join("agents").join("main").join("wire.jsonl");
    if let Some(text) = read_text_capped(&wire, JSONL_BYTE_CAP) {
        collect_kimi_wire_md(&text, project_cwd, session_dir, &mut docs);
    }
    finish_docs(docs)
}

fn collect_md_dir(dir: &Path, project_cwd: &str, source: &str, docs: &mut HashMap<String, SessionDoc>) {
    let entries = match fs::read_dir(dir) {
        Ok(iter) => iter,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            push_doc(docs, path, project_cwd, source);
        }
    }
}

fn collect_md_tree(dir: &Path, project_cwd: &str, source: &str, docs: &mut HashMap<String, SessionDoc>) {
    let entries = match fs::read_dir(dir) {
        Ok(iter) => iter,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md_tree(&path, project_cwd, source, docs);
        } else if path.is_file() {
            push_doc(docs, path, project_cwd, source);
        }
    }
}

fn collect_hunk_md(
    text: &str,
    session_id: &str,
    project_cwd: &str,
    session_dir: &Path,
    docs: &mut HashMap<String, SessionDoc>,
) {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(id) = first_string(&value, &["sessionId", "session_id"]) {
            if id != session_id {
                continue;
            }
        }
        let path = match first_string(&value, &["filePath", "file_path", "path"]) {
            Some(p) => p,
            None => continue,
        };
        if !is_markdown_path(&path) {
            continue;
        }
        let file = PathBuf::from(&path);
        if !path_under_session_or_project(&file, session_dir, project_cwd) {
            continue;
        }
        push_doc(docs, file, project_cwd, "grokbuild");
    }
}

fn collect_kimi_wire_md(
    text: &str,
    project_cwd: &str,
    session_dir: &Path,
    docs: &mut HashMap<String, SessionDoc>,
) {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event = if value.get("type").and_then(|v| v.as_str()) == Some("context.append_loop_event") {
            value.get("event").cloned().unwrap_or(Value::Null)
        } else {
            value
        };
        let ev_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let name = first_string(&event, &["name"]).unwrap_or_default();
        if ev_type != "tool.call" && event.get("type").and_then(|v| v.as_str()) != Some("tool.call") {
            if ev_type != "plan_mode.enter" {
                continue;
            }
        }
        let write_like = matches!(
            name.to_ascii_lowercase().as_str(),
            "write" | "edit" | "create"
        );
        if ev_type == "plan_mode.enter" {
            if let Some(id) = first_string(&event, &["id"]) {
                let plan = session_dir
                    .join("agents")
                    .join("main")
                    .join("plans")
                    .join(format!("{id}.md"));
                push_doc(docs, plan, project_cwd, "kimi");
            }
            continue;
        }
        if !write_like {
            continue;
        }
        let args = event.get("args").cloned().unwrap_or_else(|| {
            event
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Null)
        });
        if let Some(path) = first_string(&args, &["path", "file_path", "target_file", "filePath"]) {
            if is_markdown_path(&path) {
                let file = PathBuf::from(path);
                if path_under_session_or_project(&file, session_dir, project_cwd) {
                    push_doc(docs, file, project_cwd, "kimi");
                }
            }
        }
    }
}

fn push_doc(docs: &mut HashMap<String, SessionDoc>, path: PathBuf, project_cwd: &str, source: &str) {
    if !is_markdown_path(&path.to_string_lossy()) || !path.is_file() {
        return;
    }
    let key = normalize_path(&path.to_string_lossy());
    if docs.contains_key(&key) {
        return;
    }
    docs.insert(
        key,
        SessionDoc {
            kind: doc_kind(&path),
            title: doc_title(&path),
            rel_path: rel_project_path(&path, project_cwd),
            path: path.to_string_lossy().into_owned(),
            updated_at: file_mtime_millis(&path),
            source: source.to_string(),
        },
    );
}

fn finish_docs(docs: HashMap<String, SessionDoc>) -> Vec<SessionDoc> {
    let mut rows: Vec<SessionDoc> = docs.into_values().collect();
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.title.cmp(&b.title)));
    rows.truncate(MAX_DOCS);
    rows
}

fn doc_title(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "文档".into());
    if stem.eq_ignore_ascii_case("plan") {
        "Plan".into()
    } else {
        stem
    }
}

fn doc_kind(path: &Path) -> String {
    let norm = normalize_path(&path.to_string_lossy());
    if norm.ends_with("/plan.md") || norm.contains("/plans/") || norm.contains("/plan/") {
        "plan".into()
    } else if norm.contains("/specs/") {
        "spec".into()
    } else {
        "doc".into()
    }
}

fn is_markdown_path(path: &str) -> bool {
    normalize_path(path).ends_with(".md")
}

fn rel_project_path(path: &Path, project_cwd: &str) -> Option<String> {
    let file = normalize_path(&path.to_string_lossy());
    let root = normalize_path(project_cwd);
    if file == root {
        return None;
    }
    let prefix = format!("{root}/");
    file.strip_prefix(&prefix).map(|s| s.to_string())
}

fn path_under_session_or_project(path: &Path, session_dir: &Path, project_cwd: &str) -> bool {
    let file = normalize_path(&path.to_string_lossy());
    let session = normalize_path(&session_dir.to_string_lossy());
    let project = normalize_path(project_cwd);
    file == session
        || file.starts_with(&(session + "/"))
        || file == project
        || file.starts_with(&(project + "/"))
}

fn path_allowed(path: &Path, project: &Project) -> bool {
    let file = normalize_path(&path.to_string_lossy());
    let mut roots = vec![normalize_path(&project.path)];
    roots.push(normalize_path(&tools::grok_home().to_string_lossy()));
    roots.push(normalize_path(&tools::kimi_home().to_string_lossy()));
    roots.into_iter().any(|root| file == root || file.starts_with(&(root + "/")))
}

fn read_text_capped(path: &Path, max_bytes: u64) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len <= max_bytes {
        let mut text = String::new();
        file.read_to_string(&mut text).ok()?;
        return Some(text);
    }
    let mut buf = vec![0u8; max_bytes as usize];
    file.rewind().ok()?;
    let n = file.read(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf[..n]).into_owned())
}

pub(crate) fn parse_grok_turns(text: &str) -> Vec<SessionTurn> {
    let mut turns = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if value.get("synthetic_reason").is_some() {
            continue;
        }
        let role = match kind {
            "user" => "user",
            "assistant" => "assistant",
            _ => continue,
        };
        let Some(body) = json_text(value.get("content").unwrap_or(&Value::Null)) else {
            continue;
        };
        let text = if role == "user" {
            match clean_user_text(&body) {
                Some(t) => t,
                None => continue,
            }
        } else {
            let trimmed = body.trim();
            if trimmed.is_empty() {
                continue;
            }
            trimmed.to_string()
        };
        push_turn(&mut turns, role, text);
        if turns.len() >= MAX_TURNS {
            break;
        }
    }
    turns
}

pub(crate) fn parse_kimi_turns(text: &str) -> Vec<SessionTurn> {
    let mut turns = Vec::new();
    let mut asst = String::new();
    let mut asst_turn = String::new();
    let flush_asst = |turns: &mut Vec<SessionTurn>, asst: &mut String, asst_turn: &mut String| {
        let body = std::mem::take(asst).trim().to_string();
        asst_turn.clear();
        if !body.is_empty() {
            push_turn(turns, "assistant", body);
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "context.append_message" {
            let message = value.get("message").cloned().unwrap_or(Value::Null);
            let role = message.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let Some(body) = json_text(message.get("content").unwrap_or(&Value::Null)) else {
                continue;
            };
            if role == "user" {
                if let Some(text) = clean_user_text(&body) {
                    flush_asst(&mut turns, &mut asst, &mut asst_turn);
                    push_turn(&mut turns, "user", text);
                }
            } else if role == "assistant" {
                let trimmed = body.trim();
                if !trimmed.is_empty() {
                    if !asst.is_empty() {
                        asst.push('\n');
                    }
                    asst.push_str(trimmed);
                }
            }
        } else if kind == "turn.prompt" {
            if let Some(body) = json_text(value.get("input").unwrap_or(&Value::Null)) {
                if let Some(text) = clean_user_text(&body) {
                    flush_asst(&mut turns, &mut asst, &mut asst_turn);
                    push_turn(&mut turns, "user", text);
                }
            }
        } else if kind == "context.append_loop_event" || kind == "tool.call" {
            let event = if kind == "context.append_loop_event" {
                value.get("event").cloned().unwrap_or(Value::Null)
            } else {
                value.clone()
            };
            let ev_type = event.get("type").and_then(|v| v.as_str()).unwrap_or(kind);
            if ev_type == "content.part" {
                let part = event.get("part").cloned().unwrap_or(Value::Null);
                if part.get("type").and_then(|v| v.as_str()) == Some("text") {
                    if let Some(body) = json_text(part.get("text").unwrap_or(&Value::Null)) {
                        let turn_id = first_string(&event, &["turnId", "turn_id"]).unwrap_or_default();
                        if !asst_turn.is_empty() && turn_id != asst_turn && !asst.is_empty() {
                            flush_asst(&mut turns, &mut asst, &mut asst_turn);
                        }
                        if asst_turn.is_empty() {
                            asst_turn = turn_id;
                        }
                        let trimmed = body.trim();
                        if !trimmed.is_empty() {
                            if !asst.is_empty() {
                                asst.push('\n');
                            }
                            asst.push_str(trimmed);
                        }
                    }
                }
            } else if ev_type == "step.end" || ev_type == "turn.ended" {
                flush_asst(&mut turns, &mut asst, &mut asst_turn);
            }
        } else if kind == "turn.ended" {
            flush_asst(&mut turns, &mut asst, &mut asst_turn);
        }
        if turns.len() >= MAX_TURNS {
            break;
        }
    }
    flush_asst(&mut turns, &mut asst, &mut asst_turn);
    turns.truncate(MAX_TURNS);
    turns
}

fn push_turn(turns: &mut Vec<SessionTurn>, role: &str, text: String) {
    if turns.len() >= MAX_TURNS {
        return;
    }
    let mut text = text;
    if text.chars().count() > MAX_TURN_CHARS {
        text = text.chars().take(MAX_TURN_CHARS).collect();
    }
    let excerpt = truncate_title(&text, 120);
    turns.push(SessionTurn {
        id: format!("t-{}", turns.len()),
        role: role.to_string(),
        excerpt,
        text,
    });
}

fn json_text(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        let t = s.trim();
        return (!t.is_empty()).then(|| s.to_string());
    }
    if let Some(arr) = value.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                if !s.trim().is_empty() {
                    parts.push(s.to_string());
                }
                continue;
            }
            if let Some(s) = first_string(item, &["text", "content"]) {
                parts.push(s);
            }
        }
        let joined = parts.join("\n");
        return (!joined.trim().is_empty()).then_some(joined);
    }
    first_string(value, &["text", "content"])
}

fn extract_tag(text: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let start = text.find(&start_tag)? + start_tag.len();
    let end = text[start..].find(&end_tag)? + start;
    Some(text[start..end].to_string())
}

fn clean_user_text(text: &str) -> Option<String> {
    if let Some(query) = extract_tag(text, "user_query") {
        let query = query.trim();
        if !query.is_empty() {
            return Some(query.to_string());
        }
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("<system-reminder>")
        || trimmed.starts_with("<user_info>")
        || trimmed.starts_with("<rules>")
        || trimmed.starts_with("<git-context>")
    {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Project;
    use crate::tools::parse_time_value;
    use std::io::Write;
    use tempfile::tempdir;

    fn project(path: &Path) -> Project {
        Project {
            id: "p1".into(),
            name: "demo".into(),
            path: path.to_string_lossy().into_owned(),
            proxy_enabled: false,
            proxy_url: String::new(),
            created_at: String::new(),
        }
    }

    #[test]
    fn grok_turns_skip_system_and_extract_user_query() {
        let text = r#"
{"type":"system","content":"ignore"}
{"type":"user","content":[{"type":"text","text":"<user_info>\nOS</user_info>"}]}
{"type":"user","synthetic_reason":"system_reminder","content":[{"type":"text","text":"<user_query>hidden</user_query>"}]}
{"type":"user","content":[{"type":"text","text":"<user_query>\n修圆角\n</user_query>"}]}
{"type":"reasoning","content":"think"}
{"type":"assistant","content":"已经改好了"}
{"type":"assistant","content":""}
"#;
        let turns = parse_grok_turns(text);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].role, "user");
        assert_eq!(turns[0].text, "修圆角");
        assert_eq!(turns[1].role, "assistant");
        assert_eq!(turns[1].text, "已经改好了");
        assert!(turns[0].excerpt.contains("修圆角"));
    }

    #[test]
    fn kimi_turns_merge_text_parts() {
        let text = r#"
{"type":"context.append_message","message":{"role":"user","content":[{"type":"text","text":"看一下门户"}]}}
{"type":"context.append_message","message":{"role":"user","content":[{"type":"text","text":"<system-reminder> x"}]}}
{"type":"context.append_loop_event","event":{"type":"content.part","turnId":"t1","part":{"type":"think","think":"..."}}}
{"type":"context.append_loop_event","event":{"type":"content.part","turnId":"t1","part":{"type":"text","text":"先读配置"}}}
{"type":"context.append_loop_event","event":{"type":"content.part","turnId":"t1","part":{"type":"text","text":"再改标题"}}}
{"type":"turn.ended"}
"#;
        let turns = parse_kimi_turns(text);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].text, "看一下门户");
        assert_eq!(turns[1].role, "assistant");
        assert!(turns[1].text.contains("先读配置"));
        assert!(turns[1].text.contains("再改标题"));
    }

    #[test]
    fn grok_docs_from_plan_and_hunk_records() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().join("proj");
        let session = dir.path().join("session");
        let plans = project_root.join("docs").join("superpowers").join("plans");
        fs::create_dir_all(&plans).unwrap();
        fs::create_dir_all(&session).unwrap();
        fs::write(session.join("plan.md"), "# Plan\n").unwrap();
        let skill = plans.join("2026-09-11-rail.md");
        fs::write(&skill, "# rail\n").unwrap();
        let unrelated = project_root.join("docs").join("unrelated.md");
        fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
        fs::write(&unrelated, "nope").unwrap();
        let mut hunk = fs::File::create(session.join("hunk_records.jsonl")).unwrap();
        writeln!(
            hunk,
            r#"{{"filePath":"{}","sessionId":"sid-1"}}"#,
            skill.to_string_lossy().replace('\\', "\\\\")
        )
        .unwrap();

        let docs = collect_grok_docs(&session, "sid-1", &project_root.to_string_lossy());
        let titles: Vec<_> = docs.iter().map(|d| d.title.as_str()).collect();
        assert!(titles.contains(&"Plan"));
        assert!(titles.contains(&"2026-09-11-rail"));
        assert!(!titles.iter().any(|t| *t == "unrelated"));
        let rail = docs.iter().find(|d| d.title == "2026-09-11-rail").unwrap();
        assert_eq!(rail.kind, "plan");
        assert_eq!(
            rail.rel_path.as_deref(),
            Some("docs/superpowers/plans/2026-09-11-rail.md")
        );
    }

    #[test]
    fn kimi_docs_from_plans_dir_and_wire() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().join("proj");
        let session = dir.path().join("session");
        let plans = session.join("agents").join("main").join("plans");
        let versions = session
            .join("agents")
            .join("main")
            .join("plan")
            .join("batgirl-kid-flash-hawkeye");
        fs::create_dir_all(&plans).unwrap();
        fs::create_dir_all(&versions).unwrap();
        fs::create_dir_all(&project_root).unwrap();
        fs::write(plans.join("batgirl-kid-flash-hawkeye.md"), "# p\n").unwrap();
        fs::write(versions.join("v1.md"), "# v1\n").unwrap();
        let extra = project_root.join("docs").join("note.md");
        fs::create_dir_all(extra.parent().unwrap()).unwrap();
        fs::write(&extra, "note").unwrap();
        fs::write(
            session.join("agents").join("main").join("wire.jsonl"),
            format!(
                "{}\n{}\n",
                r#"{"type":"plan_mode.enter","id":"batgirl-kid-flash-hawkeye"}"#,
                format!(
                    r#"{{"type":"context.append_loop_event","event":{{"type":"tool.call","name":"Write","args":{{"path":"{}"}}}}}}"#,
                    extra.to_string_lossy().replace('\\', "\\\\")
                )
            ),
        )
        .unwrap();

        let docs = collect_kimi_docs(&session, &project_root.to_string_lossy());
        let titles: Vec<_> = docs.iter().map(|d| d.title.as_str()).collect();
        assert!(titles.contains(&"batgirl-kid-flash-hawkeye"));
        assert!(titles.contains(&"v1"));
        assert!(titles.contains(&"note"));
    }

    #[test]
    fn read_rejects_outside_project_and_homes() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().join("proj");
        fs::create_dir_all(&project_root).unwrap();
        let outside = dir.path().join("secret.md");
        fs::write(&outside, "nope").unwrap();
        let err = read_session_doc_for(&project(project_root.as_path()), &outside.to_string_lossy())
            .unwrap_err();
        assert!(err.contains("只能读取"));
    }

    #[test]
    fn parse_time_from_hunk_is_optional() {
        let ts = parse_time_value(&Value::String("2026-08-12T01:38:47.527606400Z".into()));
        assert!(ts.is_some());
    }
}
