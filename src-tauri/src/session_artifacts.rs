//! Session-owned Markdown references and durable, per-user-turn assistant outputs.
use crate::{
    path_norm::normalize_path,
    session_cite::{self, SessionDoc, SessionDocBody},
    state::Project,
    tools::{self, ToolId},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager};

const PREFIX: &str = "agent-dock:round:";
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Round {
    id: String,
    number: usize,
    prompt: String,
    text: String,
    updated_at: i64,
    complete: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Archive {
    source: String,
    cursor: u64,
    modified: i64,
    source_revision: String,
    header: String,
    header_len: u64,
    current: Option<String>,
    rounds: Vec<Round>,
    documents: BTreeMap<String, SessionDoc>,
}

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn project_storage(root: &Path, project: &Project) -> PathBuf {
    root.join(digest(&normalize_path(&project.path)))
}
fn archive_path(root: &Path, project: &Project, tool: ToolId, session: &str) -> PathBuf {
    project_storage(root, project).join(format!(
        "{}.json",
        digest(&format!(
            "{}\0{}\0{}",
            normalize_path(&project.path),
            tool.as_str(),
            session
        ))
    ))
}
fn storage(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|p| p.join("session-artifacts"))
        .map_err(|e| format!("无法定位会话文档存储目录：{e}"))
}
fn load(path: &Path) -> Result<Archive, String> {
    if !path.exists() {
        return Ok(Archive::default());
    }
    let bytes = fs::read(path).map_err(|e| format!("读取会话归档失败：{e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("会话归档格式错误，原文件已保留：{e}"))
}
fn save(path: &Path, archive: &Archive) -> Result<(), String> {
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| format!("创建会话归档目录失败：{e}"))?;
    let bytes = serde_json::to_vec(archive).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)
        .and_then(|_| fs::rename(&tmp, path))
        .map_err(|e| format!("保存会话总结失败：{e}"))
}

fn source(project: &Project, tool: ToolId, session: &str) -> Option<PathBuf> {
    match tool {
        ToolId::Claude => tools::find_claude_session_file(&project.path, session),
        ToolId::Grokbuild => tools::find_grok_session_dir(&project.path, session)
            .map(|p| p.join("chat_history.jsonl")),
        ToolId::Kimi => {
            tools::find_kimi_session_dir(session).map(|p| p.join("agents/main/wire.jsonl"))
        }
        ToolId::Pi => tools::find_pi_session_file(&project.path, session),
        ToolId::Opencode => Some(
            std::env::var_os("OPENCODE_DB")
                .map(PathBuf::from)
                .map(|p| {
                    if p.is_absolute() {
                        p
                    } else {
                        tools::opencode_home().join(p)
                    }
                })
                .unwrap_or_else(|| tools::opencode_home().join("opencode.db")),
        ),
        _ => None,
    }
}

pub fn list(
    app: &AppHandle,
    project: &Project,
    tool: ToolId,
    session: &str,
) -> Result<Vec<SessionDoc>, String> {
    list_in(
        &storage(app)?,
        project,
        tool,
        session,
        source(project, tool, session).as_deref(),
    )
}

fn list_in(
    root: &Path,
    project: &Project,
    tool: ToolId,
    session: &str,
    log: Option<&Path>,
) -> Result<Vec<SessionDoc>, String> {
    if session.trim().is_empty() {
        return Err("请先选择会话。".into());
    }
    let _lock = LOCK.lock().map_err(|_| "会话归档暂不可用。")?;
    let path = archive_path(root, project, tool, session);
    let legacy = root.join(path.file_name().unwrap());
    let migrated = !path.exists() && legacy.is_file();
    let mut archive = load(if migrated { &legacy } else { &path })?;
    let mut changed = migrated;
    // Retain native plan files as well as the broader references discovered below.
    if let Ok(native) = session_cite::list_session_docs_for(project, tool, session) {
        for doc in native.into_iter().filter(|d| d.kind != "summary") {
            let key = normalize_path(&doc.path);
            if archive.documents.get(&key) != Some(&doc) {
                archive.documents.insert(key, doc);
                changed = true;
            }
        }
    }
    if tool == ToolId::Opencode {
        if let Some(db) = log.filter(|p| p.is_file()) {
            changed |= collect_opencode(&mut archive, db, project, session)?;
        }
    }
    if let Some(log) = log.filter(|p| p.is_file() && tool != ToolId::Opencode) {
        let meta = log.metadata().map_err(|e| e.to_string())?;
        let modified = tools::file_mtime_millis(log);
        let name = normalize_path(&log.to_string_lossy());
        let replaced =
            archive.header_len > 0 && fingerprint(log, archive.header_len)? != archive.header;
        if archive.source != name
            || archive.cursor > meta.len()
            || replaced
            || (archive.cursor == meta.len() && archive.modified != modified)
        {
            archive.cursor = 0;
            archive.current = None;
        }
        if archive.source != name || archive.cursor != meta.len() || archive.modified != modified {
            let mut file = fs::File::open(log).map_err(|e| e.to_string())?;
            file.seek(SeekFrom::Start(archive.cursor))
                .map_err(|e| e.to_string())?;
            let mut reader = BufReader::new(file);
            let mut line = Vec::new();
            let mut mentions = HashSet::new();
            loop {
                line.clear();
                let offset = archive.cursor;
                let bytes = reader
                    .read_until(b'\n', &mut line)
                    .map_err(|e| format!("读取会话记录失败：{e}"))?;
                if bytes == 0 {
                    break;
                }
                match serde_json::from_slice::<Value>(&line) {
                    Ok(value) => {
                        collect_strings(&value, &mut mentions);
                        consume(&mut archive, &value, tool, offset);
                    }
                    Err(_) if !line.ends_with(b"\n") => break, // Resume at this offset once the writer completes the line.
                    Err(_) => {}
                }
                archive.cursor += bytes as u64;
            }
            collect_references(
                &mut archive,
                project,
                log.parent().unwrap_or(Path::new(&project.path)),
                tool,
                mentions,
            );
            archive.source = name;
            archive.modified = modified;
            archive.header_len = archive.cursor.min(4096);
            archive.header = fingerprint(log, archive.header_len)?;
            changed = true;
        }
    }
    if changed {
        save(&path, &archive)?;
    }
    let mut docs: Vec<_> = archive
        .documents
        .into_values()
        .map(|mut doc| {
            doc.exists = Some(Path::new(&doc.path).is_file());
            if doc.exists == Some(true) {
                doc.updated_at = tools::file_mtime_millis(Path::new(&doc.path));
            }
            doc
        })
        .collect();
    docs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.path.cmp(&b.path)));
    let summaries = archive
        .rounds
        .into_iter()
        .filter(|round| !round.text.trim().is_empty())
        .rev()
        .map(|round| SessionDoc {
            kind: "summary".into(),
            title: format!(
                "第 {} 轮 · {}",
                round.number,
                tools::truncate_title(&round.prompt, 40)
            ),
            path: format!("{PREFIX}{}/{session}/{}", tool.as_str(), round.id),
            rel_path: None,
            updated_at: round.updated_at,
            source: tool.as_str().into(),
            round: Some(round.number),
            excerpt: Some(tools::truncate_title(&round.text, 200)),
            exists: Some(true),
        });
    Ok(summaries.chain(docs).collect())
}

fn fingerprint(path: &Path, len: u64) -> Result<String, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(len).read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn collect_opencode(
    archive: &mut Archive,
    db: &Path,
    project: &Project,
    session: &str,
) -> Result<bool, String> {
    let connection =
        rusqlite::Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("读取 OpenCode 会话库失败：{e}"))?;
    connection
        .busy_timeout(std::time::Duration::from_millis(1000))
        .map_err(|e| e.to_string())?;
    let directory: String = connection
        .query_row(
            "SELECT directory FROM session WHERE id = ?1",
            [session],
            |r| r.get(0),
        )
        .map_err(|_| "未找到当前项目的 OpenCode 会话。")?;
    if !tools::matches_cwd(&directory, &project.path) {
        return Err("此 OpenCode 会话不属于当前项目。".into());
    }
    let revision: String = connection.query_row(
        "SELECT COALESCE(MAX(m.time_updated),0) || ':' || COALESCE(MAX(p.time_updated),0) || ':' || COUNT(p.id) || ':' || COALESCE(SUM(LENGTH(p.data)),0) FROM message m LEFT JOIN part p ON p.message_id = m.id WHERE m.session_id = ?1", [session], |r| r.get(0)
    ).map_err(|e| format!("读取 OpenCode 会话索引失败：{e}"))?;
    if archive.source == normalize_path(&db.to_string_lossy())
        && archive.source_revision == revision
    {
        return Ok(false);
    }
    let mut statement = connection.prepare("SELECT m.id, m.time_created, m.data, p.data FROM message m LEFT JOIN part p ON p.message_id = m.id WHERE m.session_id = ?1 ORDER BY m.time_created, m.id, p.time_created, p.id")
        .map_err(|e| format!("读取 OpenCode 消息失败：{e}"))?;
    let mut rows = statement.query([session]).map_err(|e| e.to_string())?;
    let mut pending: Option<(String, i64, Value, Vec<Value>)> = None;
    let mut mentions = HashSet::new();
    archive.current = None;
    let flush = |archive: &mut Archive,
                 pending: (String, i64, Value, Vec<Value>),
                 mentions: &mut HashSet<String>| {
        let (id, at, info, parts) = pending;
        for part in &parts {
            collect_strings(part, mentions);
        }
        let role = info.get("role").and_then(Value::as_str).unwrap_or("");
        let last_tool = parts
            .iter()
            .rposition(|p| p.get("type").and_then(Value::as_str) == Some("tool"));
        let content: Vec<Value> = parts
            .iter()
            .enumerate()
            .filter(|(i, part)| {
                part.get("type").and_then(Value::as_str) == Some("text")
                    && part.get("synthetic").and_then(Value::as_bool) != Some(true)
                    && (role != "assistant" || last_tool.is_none_or(|last| *i > last))
            })
            .map(|(_, part)| part.clone())
            .collect();
        let value = serde_json::json!({"type":role,"uuid":id,"timestamp":at,"message":{"content":content,"stop_reason":if info.get("finish").and_then(Value::as_str) == Some("stop") { "end_turn" } else { "" }}});
        if role == "assistant" && content.is_empty() && last_tool.is_some() {
            tool_call(archive);
        } else {
            consume(archive, &value, ToolId::Opencode, 0);
        }
    };
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let id: String = row.get(0).map_err(|e| e.to_string())?;
        if pending.as_ref().is_some_and(|p| p.0 != id) {
            flush(archive, pending.take().unwrap(), &mut mentions);
        }
        if pending.is_none() {
            let at: i64 = row.get(1).map_err(|e| e.to_string())?;
            let text: String = row.get(2).map_err(|e| e.to_string())?;
            pending = Some((
                id,
                at,
                serde_json::from_str(&text).unwrap_or(Value::Null),
                Vec::new(),
            ));
        }
        if let Some(text) = row.get::<_, Option<String>>(3).map_err(|e| e.to_string())? {
            if let Ok(part) = serde_json::from_str(&text) {
                pending.as_mut().unwrap().3.push(part);
            }
        }
    }
    if let Some(pending) = pending {
        flush(archive, pending, &mut mentions);
    }
    collect_references(
        archive,
        project,
        db.parent().unwrap_or(Path::new(&project.path)),
        ToolId::Opencode,
        mentions,
    );
    archive.source = normalize_path(&db.to_string_lossy());
    archive.source_revision = revision;
    Ok(true)
}

pub fn read(
    app: &AppHandle,
    project: &Project,
    path: &str,
) -> Result<Option<SessionDocBody>, String> {
    read_in(&storage(app)?, project, path)
}
pub fn is_referenced(app: &AppHandle, project: &Project, path: &Path) -> Result<bool, String> {
    referenced_in(&storage(app)?, project, path)
}
fn referenced_in(root: &Path, project: &Project, path: &Path) -> Result<bool, String> {
    if !path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        return Ok(false);
    }
    let _lock = LOCK.lock().map_err(|_| "会话归档暂不可用。")?;
    let key = normalize_path(&path.to_string_lossy());
    let Ok(entries) = fs::read_dir(project_storage(root, project)) else {
        return Ok(false);
    };
    for entry in entries.flatten() {
        if entry.path().extension().is_some_and(|ext| ext == "json") {
            if let Ok(archive) = load(&entry.path()) {
                if archive.documents.contains_key(&key) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
fn read_in(root: &Path, project: &Project, path: &str) -> Result<Option<SessionDocBody>, String> {
    let Some(rest) = path.strip_prefix(PREFIX) else {
        return Ok(None);
    };
    let mut parts = rest.splitn(3, '/');
    let tool = parts
        .next()
        .and_then(ToolId::parse)
        .ok_or("总结引用无效。")?;
    let session = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or("总结引用无效。")?;
    let id = parts.next().ok_or("总结引用无效。")?;
    let _lock = LOCK.lock().map_err(|_| "会话归档暂不可用。")?;
    let archive = load(&archive_path(root, project, tool, session))?;
    let round = archive
        .rounds
        .iter()
        .find(|r| r.id == id)
        .ok_or("未找到这轮总结，请刷新文档列表。")?;
    Ok(Some(SessionDocBody {
        path: path.into(),
        title: format!("第 {} 轮总结", round.number),
        rel_path: None,
        text: round.text.clone(),
    }))
}

fn plain(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return (!text.trim().is_empty()).then(|| text.trim().to_string());
    }
    let parts: Vec<_> = value
        .as_array()?
        .iter()
        .filter_map(|part| {
            if part.get("type").and_then(Value::as_str) == Some("text") {
                part.get("text").and_then(Value::as_str)
            } else {
                part.as_str()
            }
        })
        .filter(|s| !s.trim().is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n"))
}
fn timestamp(value: &Value) -> i64 {
    [
        value.get("timestamp"),
        value.get("time"),
        value.get("createdAt"),
        value.get("message").and_then(|m| m.get("timestamp")),
    ]
    .into_iter()
    .flatten()
    .find_map(tools::parse_time_value)
    .unwrap_or(0)
}
fn user(archive: &mut Archive, value: &Value, prompt: String, offset: u64) {
    let prompt = prompt
        .split_once("<user_query>")
        .and_then(|(_, s)| s.split_once("</user_query>"))
        .map(|(s, _)| s.trim().to_string())
        .unwrap_or(prompt);
    if [
        "<system-reminder>",
        "<user_info>",
        "<rules>",
        "<git-context>",
        "[Request interrupted by user",
    ]
    .iter()
    .any(|prefix| prompt.starts_with(prefix))
    {
        return;
    }
    if let Some(id) = &archive.current {
        if let Some(round) = archive.rounds.iter_mut().find(|r| &r.id == id) {
            round.complete = true;
        }
    }
    let raw_id = tools::first_string(value, &["uuid", "id", "turnId", "turn_id"])
        .unwrap_or_else(|| format!("{offset}:{prompt}"));
    let id = digest(&raw_id);
    if !archive.rounds.iter().any(|r| r.id == id) {
        archive.rounds.push(Round {
            id: id.clone(),
            number: archive.rounds.len() + 1,
            prompt,
            updated_at: timestamp(value),
            ..Round::default()
        });
    }
    if let Some(round) = archive.rounds.iter_mut().find(|r| r.id == id) {
        round.complete = false;
    }
    archive.current = Some(id);
}
fn assistant(archive: &mut Archive, text: String, at: i64, append: bool, complete: bool) {
    if archive.current.is_none() {
        user(archive, &Value::Null, "会话输出".into(), 0);
    }
    if let Some(round) = archive
        .rounds
        .iter_mut()
        .find(|r| Some(&r.id) == archive.current.as_ref())
    {
        if append && !round.text.is_empty() {
            round.text.push('\n');
            round.text.push_str(&text);
        } else {
            round.text = text;
        }
        round.updated_at = round.updated_at.max(at);
        round.complete = complete;
    }
}
fn tool_call(archive: &mut Archive) {
    if let Some(round) = archive
        .rounds
        .iter_mut()
        .find(|r| Some(&r.id) == archive.current.as_ref())
    {
        if !round.complete {
            round.text.clear();
        }
    }
}
fn consume(archive: &mut Archive, value: &Value, tool: ToolId, offset: u64) {
    if ["isMeta", "isSidechain", "isCompactSummary"]
        .into_iter()
        .any(|key| value.get(key).and_then(Value::as_bool) == Some(true))
        || value.get("synthetic_reason").is_some()
    {
        return;
    }
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let message = value.get("message").unwrap_or(value);
    if tool == ToolId::Kimi {
        let event = value.get("event").unwrap_or(value);
        let ev_type = event.get("type").and_then(Value::as_str).unwrap_or("");
        if kind == "turn.prompt" {
            if let Some(text) = plain(value.get("input").unwrap_or(&Value::Null)) {
                user(archive, value, text, offset);
            }
        } else if kind == "context.append_message" {
            match message.get("role").and_then(Value::as_str) {
                Some("user") => {
                    if let Some(text) = plain(message.get("content").unwrap_or(&Value::Null)) {
                        // Kimi records the same prompt in both turn.prompt and append_message.
                        let duplicate = archive.rounds.iter().any(|r| {
                            Some(&r.id) == archive.current.as_ref()
                                && r.prompt == text
                                && r.text.is_empty()
                        });
                        if !duplicate {
                            user(archive, value, text, offset);
                        }
                    }
                }
                Some("assistant") => {
                    if let Some(text) = plain(message.get("content").unwrap_or(&Value::Null)) {
                        assistant(archive, text, timestamp(value), false, true);
                    }
                }
                _ => {}
            }
        } else if ev_type == "content.part"
            && event
                .get("part")
                .and_then(|p| p.get("type"))
                .and_then(Value::as_str)
                == Some("text")
        {
            if let Some(text) = event
                .get("part")
                .and_then(|p| p.get("text"))
                .and_then(Value::as_str)
            {
                assistant(archive, text.into(), timestamp(value), true, false);
            }
        } else if ev_type == "tool.call" {
            tool_call(archive);
        } else if ev_type == "turn.ended" || kind == "turn.ended" {
            if let Some(round) = archive
                .rounds
                .iter_mut()
                .find(|r| Some(&r.id) == archive.current.as_ref())
            {
                round.complete = true;
            }
        }
        return;
    }
    let role = if tool == ToolId::Pi {
        message.get("role").and_then(Value::as_str).unwrap_or("")
    } else {
        kind
    };
    if role == "user" {
        if let Some(text) = plain(message.get("content").unwrap_or(&Value::Null)) {
            user(archive, value, text, offset);
        }
    } else if role == "assistant" {
        let content = message.get("content").unwrap_or(&Value::Null);
        let has_tool = content.as_array().is_some_and(|a| {
            a.iter().any(|p| {
                matches!(
                    p.get("type").and_then(Value::as_str),
                    Some("tool_use" | "toolCall")
                )
            })
        }) || message
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if has_tool {
            tool_call(archive);
            return;
        }
        if let Some(text) = plain(content) {
            let complete = message.get("stop_reason").and_then(Value::as_str) == Some("end_turn")
                || message.get("stopReason").and_then(Value::as_str) == Some("stop");
            assistant(archive, text, timestamp(value), false, complete);
        }
    }
}

fn decode_path(raw: &str) -> String {
    let raw = raw
        .trim()
        .trim_start_matches('@')
        .trim_start_matches("file://");
    let raw = if raw.starts_with('/') && raw.as_bytes().get(2) == Some(&b':') {
        &raw[1..]
    } else {
        raw
    };
    let bytes = raw.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            if let Ok(n) = u8::from_str_radix(&raw[i + 1..i + 3], 16) {
                out.push(n);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn mentions(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    // Explicit Markdown targets and quoted paths retain spaces in file names.
    for (open, close) in [
        ("`", "`"),
        ("\"", "\""),
        ("'", "'"),
        ("](", ")"),
        ("<", ">"),
    ] {
        let mut rest = text;
        while let Some((_, after)) = rest.split_once(open) {
            let Some((candidate, tail)) = after.split_once(close) else {
                break;
            };
            let candidate = candidate.trim().trim_matches(['<', '>']);
            let lower = candidate.to_ascii_lowercase();
            if let Some(end) = lower.rfind(".md") {
                let next = lower.as_bytes().get(end + 3).copied();
                if next.is_none() || matches!(next, Some(b'#' | b':' | b'?' | b' ')) {
                    found.push(candidate[..end + 3].into());
                }
            }
            rest = tail;
        }
    }
    for word in text.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '`' | '"'
                    | '\''
                    | '<'
                    | '>'
                    | '['
                    | ']'
                    | '('
                    | ')'
                    | '，'
                    | '。'
                    | '、'
                    | '；'
                    | ';'
                    | '|'
                    | '='
            )
    }) {
        let lower = word.to_ascii_lowercase();
        if let Some(end) = lower.find(".md") {
            let next = lower.as_bytes().get(end + 3).copied();
            if next.is_none() || matches!(next, Some(b'#' | b':' | b'?' | b',' | b'.')) {
                found.push(word[..end + 3].into());
            }
        }
    }
    found
        .into_iter()
        .filter(|p| !p.starts_with("http:") && !p.starts_with("https:") && !p.contains('\n'))
        .map(|p| decode_path(&p))
        .collect()
}
fn collect_strings(value: &Value, paths: &mut HashSet<String>) {
    match value {
        Value::String(text) => {
            paths.extend(mentions(text));
            if text.starts_with('{') {
                if let Ok(value) = serde_json::from_str::<Value>(text) {
                    collect_strings(&value, paths);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_strings(item, paths);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                if !matches!(key.as_str(), "thinking" | "signature" | "data") {
                    collect_strings(item, paths);
                }
            }
        }
        _ => {}
    }
}
fn lexical(path: PathBuf) -> PathBuf {
    let mut clean = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                clean.pop();
            }
            Component::CurDir => {}
            _ => clean.push(part),
        }
    }
    clean
}
fn collect_references(
    archive: &mut Archive,
    project: &Project,
    session_dir: &Path,
    tool: ToolId,
    mentions: HashSet<String>,
) {
    let names: HashSet<_> = mentions
        .iter()
        .filter(|name| {
            Path::new(name).components().count() == 1
                && !name.contains(['/', '\\'])
                && !Path::new(&project.path).join(name).is_file()
                && !session_dir.join(name).is_file()
        })
        .map(|name| name.to_lowercase())
        .collect();
    let mut matches: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    // A bare filename may refer to docs/guide.md. Resolve only mentioned names,
    // avoiding dependencies, build products and symlinked directories.
    let mut pending = if names.is_empty() {
        Vec::new()
    } else {
        vec![PathBuf::from(&project.path)]
    };
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if kind.is_dir()
                && !matches!(
                    name.as_str(),
                    ".git"
                        | ".svn"
                        | "node_modules"
                        | "target"
                        | "dist"
                        | "build"
                        | ".worktrees"
                        | ".venv"
                        | "vendor"
                )
            {
                pending.push(entry.path());
            } else if kind.is_file() && names.contains(&name) {
                matches.entry(name).or_default().push(entry.path());
            }
        }
    }
    for mention in mentions {
        if let Some(paths) = matches.get(&mention.to_lowercase()) {
            let unresolved =
                normalize_path(&Path::new(&project.path).join(&mention).to_string_lossy());
            if archive
                .documents
                .get(&unresolved)
                .is_some_and(|doc| doc.exists == Some(false))
            {
                archive.documents.remove(&unresolved);
            }
            for path in paths {
                collect_reference(archive, project, session_dir, tool, &path.to_string_lossy());
            }
        } else {
            collect_reference(archive, project, session_dir, tool, &mention);
        }
    }
}
fn collect_reference(
    archive: &mut Archive,
    project: &Project,
    session_dir: &Path,
    tool: ToolId,
    mention: &str,
) {
    let raw = if let Some(rest) = mention
        .strip_prefix("~/")
        .or_else(|| mention.strip_prefix("~\\"))
    {
        dirs::home_dir().unwrap_or_default().join(rest)
    } else {
        PathBuf::from(mention)
    };
    let path = if raw.is_absolute() {
        lexical(raw)
    } else {
        let project_path = lexical(Path::new(&project.path).join(&raw));
        if project_path.is_file() {
            project_path
        } else {
            let session_path = lexical(session_dir.join(&raw));
            if session_path.is_file() {
                session_path
            } else {
                project_path
            }
        }
    };
    let key = normalize_path(&path.to_string_lossy());
    let allowed = if path.is_file() {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    } else {
        let root = normalize_path(&project.path);
        key.starts_with(&(root + "/"))
    };
    if !allowed {
        return;
    }
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let root = normalize_path(&project.path) + "/";
    archive.documents.insert(
        key.clone(),
        SessionDoc {
            kind: if key.contains("/plans/") || stem.eq_ignore_ascii_case("plan") {
                "plan"
            } else if key.contains("/specs/") {
                "spec"
            } else {
                "doc"
            }
            .into(),
            title: stem,
            path: path.to_string_lossy().into_owned(),
            rel_path: key.strip_prefix(&root).map(str::to_string),
            updated_at: tools::file_mtime_millis(&path),
            source: tool.as_str().into(),
            round: None,
            excerpt: None,
            exists: Some(path.is_file()),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;

    fn project(root: &Path) -> Project {
        Project {
            id: "p".into(),
            name: "project".into(),
            path: root.to_string_lossy().into(),
            proxy_enabled: false,
            proxy_url: String::new(),
            created_at: String::new(),
        }
    }
    fn write_log(path: &Path, values: &[Value]) {
        fs::write(
            path,
            values
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )
        .unwrap();
    }
    fn prompt(id: &str, text: &str) -> Value {
        json!({"type":"user","uuid":id,"message":{"content":text}})
    }
    fn answer(text: &str) -> Value {
        json!({"type":"assistant","timestamp":"2026-10-29T00:00:00Z","message":{"stop_reason":"end_turn","content":[{"type":"text","text":text}]}})
    }

    #[test]
    fn every_round_survives_reload_append_and_transcript_compaction() {
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        let cache = root.path().join("cache");
        let log = root.path().join("history.jsonl");
        write_log(
            &log,
            &[
                prompt("one", "第一轮"),
                answer("第一轮总结，不应被第二轮覆盖。"),
                prompt("two", "第二轮"),
                answer("第二轮总结。"),
            ],
        );
        let docs = list_in(&cache, &p, ToolId::Claude, "session", Some(&log)).unwrap();
        let first = docs
            .iter()
            .find(|d| d.round == Some(1))
            .unwrap()
            .path
            .clone();
        assert_eq!(docs.iter().filter(|d| d.kind == "summary").count(), 2);
        assert!(read_in(&cache, &p, &first)
            .unwrap()
            .unwrap()
            .text
            .contains("第一轮总结"));
        let again = list_in(&cache, &p, ToolId::Claude, "session", Some(&log)).unwrap();
        assert_eq!(docs, again);
        // Only the latest turn remains in the CLI transcript after compaction.
        write_log(
            &log,
            &[
                prompt("two", "第二轮"),
                answer("第二轮总结。"),
                prompt("three", "第三轮"),
                answer("第三轮总结。"),
            ],
        );
        let newer = list_in(&cache, &p, ToolId::Claude, "session", Some(&log)).unwrap();
        assert_eq!(newer.iter().filter(|d| d.kind == "summary").count(), 3);
        assert!(read_in(&cache, &p, &first)
            .unwrap()
            .unwrap()
            .text
            .contains("第一轮总结"));
        fs::remove_file(&log).unwrap();
        assert_eq!(
            list_in(&cache, &p, ToolId::Claude, "session", None)
                .unwrap()
                .len(),
            3
        );
        let another = project(&root.path().join("other-project"));
        assert!(read_in(&cache, &another, &first).is_err());
    }

    #[test]
    fn references_include_mentions_links_reads_and_unicode_paths() {
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        fs::create_dir_all(root.path().join("docs")).unwrap();
        for name in ["guide.md", "design plan.md", "方案.md"] {
            fs::write(root.path().join("docs").join(name), "# document").unwrap();
        }
        let log = root.path().join("history.jsonl");
        write_log(
            &log,
            &[
                prompt(
                    "one",
                    "请看 `docs/guide.md:12`、[设计](<docs/design%20plan.md>) 和 docs/方案.md。",
                ),
                json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"docs/guide.md"}}]}}),
                answer("文档都已阅读，后续计划是 `docs/future.md`。"),
            ],
        );
        let docs = list_in(
            &root.path().join("cache"),
            &p,
            ToolId::Claude,
            "s",
            Some(&log),
        )
        .unwrap();
        let files: Vec<_> = docs.iter().filter(|d| d.kind != "summary").collect();
        assert_eq!(files.len(), 4, "{files:?}");
        assert!(files.iter().any(|d| d.title == "方案"));
        assert!(files
            .iter()
            .any(|d| d.title == "design plan" && d.exists == Some(true)));
        assert!(files
            .iter()
            .any(|d| d.title == "future" && d.exists == Some(false)));
        assert_eq!(decode_path("docs/%文.md"), "docs/%文.md");
    }

    #[test]
    fn scans_more_than_eighty_rounds_and_resumes_partial_lines() {
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        let log = root.path().join("history.jsonl");
        let values: Vec<_> = (0..110)
            .flat_map(|i| {
                [
                    prompt(&i.to_string(), &format!("第 {i} 个问题")),
                    answer(&format!("第 {i} 个问题的总结")),
                ]
            })
            .collect();
        write_log(&log, &values);
        let cache = root.path().join("cache");
        assert_eq!(
            list_in(&cache, &p, ToolId::Claude, "s", Some(&log))
                .unwrap()
                .len(),
            110
        );
        let mut file = fs::OpenOptions::new().append(true).open(&log).unwrap();
        write!(
            file,
            "{}\n{{\"type\":\"assistant\",\"message\":{{\"content\":\"",
            prompt("last", "最新一轮")
        )
        .unwrap();
        assert_eq!(
            list_in(&cache, &p, ToolId::Claude, "s", Some(&log))
                .unwrap()
                .len(),
            110
        );
        writeln!(file, "最后总结\"}}}}").unwrap();
        let docs = list_in(&cache, &p, ToolId::Claude, "s", Some(&log)).unwrap();
        assert_eq!(docs.len(), 111);
        assert_eq!(docs[0].round, Some(111));
    }

    #[test]
    fn unfinished_utf8_writes_resume_without_losing_saved_rounds() {
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        let log = root.path().join("history.jsonl");
        let cache = root.path().join("cache");
        write_log(
            &log,
            &[
                prompt("one", "第一轮"),
                answer("已经完成"),
                prompt("two", "第二轮"),
            ],
        );
        let next = answer("新的总结").to_string() + "\n";
        let split = next.find('新').unwrap() + 1;
        let mut file = fs::OpenOptions::new().append(true).open(&log).unwrap();
        file.write_all(&next.as_bytes()[..split]).unwrap();
        assert_eq!(
            list_in(&cache, &p, ToolId::Claude, "s", Some(&log))
                .unwrap()
                .len(),
            1
        );
        file.write_all(&next.as_bytes()[split..]).unwrap();
        let docs = list_in(&cache, &p, ToolId::Claude, "s", Some(&log)).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(
            read_in(&cache, &p, &docs[0].path).unwrap().unwrap().text,
            "新的总结"
        );
    }

    #[test]
    fn tool_progress_sidechains_and_tool_results_do_not_become_summaries() {
        let mut archive = Archive::default();
        consume(&mut archive, &prompt("one", "检查代码"), ToolId::Claude, 0);
        consume(
            &mut archive,
            &json!({"type":"assistant","message":{"content":"正在检查，请稍候"}}),
            ToolId::Claude,
            1,
        );
        consume(
            &mut archive,
            &json!({"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}}]}}),
            ToolId::Claude,
            2,
        );
        consume(
            &mut archive,
            &json!({"type":"user","message":{"content":[{"type":"tool_result","content":"read results"}]}}),
            ToolId::Claude,
            3,
        );
        assert_eq!(archive.rounds.len(), 1);
        assert!(archive.rounds[0].text.is_empty());
        consume(&mut archive, &answer("检查完成"), ToolId::Claude, 4);
        consume(
            &mut archive,
            &json!({"type":"assistant","isSidechain":true,"message":{"content":"后台任务输出"}}),
            ToolId::Claude,
            5,
        );
        assert_eq!(archive.rounds[0].text, "检查完成");
    }

    #[test]
    fn grok_pi_and_kimi_formats_preserve_each_user_round() {
        for tool in [ToolId::Grokbuild, ToolId::Pi, ToolId::Kimi] {
            let mut archive = Archive::default();
            let values = match tool {
                ToolId::Grokbuild => vec![
                    json!({"type":"user","id":"one","content":"重复问题"}),
                    json!({"type":"assistant","content":"第一轮结果"}),
                    json!({"type":"user","id":"two","content":"重复问题"}),
                    json!({"type":"assistant","content":[{"type":"text","text":"第二轮结果"}]}),
                ],
                ToolId::Pi => vec![
                    json!({"type":"message","id":"one","message":{"role":"user","content":"重复问题"}}),
                    json!({"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"第一轮结果"}],"stopReason":"stop"}}),
                    json!({"type":"message","id":"two","message":{"role":"user","content":"重复问题"}}),
                    json!({"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"第二轮结果"}],"stopReason":"stop"}}),
                ],
                _ => vec![
                    json!({"type":"turn.prompt","turnId":"one","input":"重复问题"}),
                    json!({"type":"context.append_message","message":{"role":"user","content":"重复问题"}}),
                    json!({"type":"context.append_loop_event","event":{"type":"content.part","part":{"type":"text","text":"中间说明"}}}),
                    json!({"type":"context.append_loop_event","event":{"type":"tool.call"}}),
                    json!({"type":"context.append_loop_event","event":{"type":"content.part","part":{"type":"text","text":"第一轮结果"}}}),
                    json!({"type":"turn.ended"}),
                    json!({"type":"turn.prompt","turnId":"two","input":"重复问题"}),
                    json!({"type":"context.append_loop_event","event":{"type":"content.part","part":{"type":"text","text":"第二轮结果"}}}),
                    json!({"type":"turn.ended"}),
                ],
            };
            for (offset, value) in values.iter().enumerate() {
                consume(&mut archive, value, tool, offset as u64);
            }
            assert_eq!(archive.rounds.len(), 2, "{tool:?}");
            assert_eq!(archive.rounds[0].text, "第一轮结果", "{tool:?}");
            assert_eq!(archive.rounds[1].text, "第二轮结果", "{tool:?}");
        }
    }

    #[test]
    fn opencode_sqlite_parts_include_final_text_and_read_references() {
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        let db = root.path().join("opencode.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE session(id TEXT PRIMARY KEY,directory TEXT); CREATE TABLE message(id TEXT PRIMARY KEY,session_id TEXT,time_created INTEGER,time_updated INTEGER,data TEXT); CREATE TABLE part(id TEXT PRIMARY KEY,message_id TEXT,time_created INTEGER,time_updated INTEGER,data TEXT);").unwrap();
        conn.execute("INSERT INTO session VALUES('s',?1)", [&p.path])
            .unwrap();
        fs::write(root.path().join("guide.md"), "# Guide").unwrap();
        for (id, at, role) in [
            ("u1", 1, "user"),
            ("a1", 2, "assistant"),
            ("u2", 3, "user"),
            ("a2", 4, "assistant"),
        ] {
            conn.execute(
                "INSERT INTO message VALUES(?1,'s',?2,?2,?3)",
                rusqlite::params![id, at, json!({"role":role,"finish":"stop"}).to_string()],
            )
            .unwrap();
        }
        for (id, message, at, data) in [
            (
                "p1",
                "u1",
                1,
                json!({"type":"text","text":"请检查 guide.md"}),
            ),
            ("p2", "a1", 2, json!({"type":"text","text":"正在阅读"})),
            (
                "p3",
                "a1",
                3,
                json!({"type":"tool","tool":"read","state":{"input":{"filePath":"guide.md"}}}),
            ),
            ("p4", "a1", 4, json!({"type":"text","text":"第一轮完成"})),
            ("p5", "u2", 5, json!({"type":"text","text":"第二个问题"})),
            ("p6", "a2", 6, json!({"type":"text","text":"第二轮完成"})),
        ] {
            conn.execute(
                "INSERT INTO part VALUES(?1,?2,?3,?3,?4)",
                rusqlite::params![id, message, at, data.to_string()],
            )
            .unwrap();
        }
        let cache = root.path().join("cache");
        let docs = list_in(&cache, &p, ToolId::Opencode, "s", Some(&db)).unwrap();
        assert_eq!(docs.iter().filter(|d| d.kind == "summary").count(), 2);
        assert!(docs.iter().any(|d| d.title == "guide"));
        let first = docs.iter().find(|d| d.round == Some(1)).unwrap();
        assert_eq!(
            read_in(&cache, &p, &first.path).unwrap().unwrap().text,
            "第一轮完成"
        );
        assert_eq!(
            docs,
            list_in(&cache, &p, ToolId::Opencode, "s", Some(&db)).unwrap()
        );
        conn.execute("DELETE FROM message WHERE id IN ('u1','a1')", [])
            .unwrap();
        assert!(list_in(&cache, &p, ToolId::Opencode, "s", Some(&db))
            .unwrap()
            .iter()
            .any(|d| d.round == Some(1)));
        assert!(list_in(
            &cache,
            &project(&root.path().join("other")),
            ToolId::Opencode,
            "s",
            Some(&db)
        )
        .is_err());
    }

    #[test]
    fn large_logs_bare_names_and_larger_rewritten_logs_keep_history() {
        let root = tempfile::tempdir().unwrap();
        let p = project(root.path());
        fs::create_dir(root.path().join("docs")).unwrap();
        fs::write(root.path().join("docs/guide.md"), "# Guide").unwrap();
        let log = root.path().join("history.jsonl");
        write_log(
            &log,
            &[
                prompt("one", "请看 guide.md"),
                answer("早期结果"),
                json!({"type":"user","message":{"content":[{"type":"tool_result","content":"x".repeat(9 * 1024 * 1024)}]}}),
                prompt("two", "第二轮"),
                answer("尾部结果"),
            ],
        );
        let cache = root.path().join("cache");
        let docs = list_in(&cache, &p, ToolId::Claude, "s", Some(&log)).unwrap();
        assert_eq!(docs.len(), 3);
        assert!(docs
            .iter()
            .any(|d| d.rel_path.as_deref() == Some("docs/guide.md")));
        write_log(
            &log,
            &[
                prompt("three", "重新压缩后的一轮"),
                answer(&"新结果".repeat(4 * 1024 * 1024)),
            ],
        );
        let docs = list_in(&cache, &p, ToolId::Claude, "s", Some(&log)).unwrap();
        assert_eq!(docs.iter().filter(|d| d.kind == "summary").count(), 3);
        let mut archive = Archive::default();
        collect_reference(
            &mut archive,
            &p,
            root.path(),
            ToolId::Claude,
            "../outside.md",
        );
        assert!(archive.documents.is_empty());
    }

    #[test]
    fn explicit_external_markdown_references_are_project_scoped() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let p = project(&workspace);
        let external = root.path().join("shared guide.MD");
        let unseen = root.path().join("unseen.md");
        fs::write(&external, "# Shared").unwrap();
        fs::write(&unseen, "# Unseen").unwrap();
        let log = workspace.join("history.jsonl");
        write_log(
            &log,
            &[
                prompt("one", &format!("请看 `{}`", external.display())),
                answer("已阅读共享文档"),
            ],
        );
        let cache = root.path().join("cache");
        let docs = list_in(&cache, &p, ToolId::Claude, "s", Some(&log)).unwrap();
        assert!(docs.iter().any(|d| d.path == external.to_string_lossy()));
        assert!(referenced_in(&cache, &p, &external).unwrap());
        assert!(!referenced_in(&cache, &p, &unseen).unwrap());
        assert!(!referenced_in(&cache, &project(root.path()), &external).unwrap());
    }
}
