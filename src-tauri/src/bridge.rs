use crate::path_norm;
use crate::platform;
use crate::state::{self, AppSettings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::AppHandle;

const INIT_TIMEOUT: Duration = Duration::from_secs(20);
const LIST_TIMEOUT: Duration = Duration::from_secs(30);
const REVIEW_TIMEOUT: Duration = Duration::from_secs(8 * 60);

pub struct BridgeHub {
    server: Mutex<Option<AppServer>>,
}

impl BridgeHub {
    pub fn new() -> Self {
        Self {
            server: Mutex::new(None),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Option<AppServer>>, String> {
        self.server
            .lock()
            .map_err(|_| "编排服务正忙，请稍后再试".into())
    }

    fn with_server<T>(
        &self,
        settings: &AppSettings,
        f: impl FnOnce(&mut AppServer) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = self.lock()?;
        let stale = match guard.as_mut() {
            Some(server) => !server.alive(),
            None => true,
        };
        if stale {
            *guard = None;
            let mut server = AppServer::spawn(settings)?;
            server.handshake()?;
            *guard = Some(server);
        }
        match f(guard.as_mut().expect("app-server just initialized")) {
            Ok(value) => Ok(value),
            Err(err) => {
                if err.contains("断开") || err.contains("无响应") {
                    *guard = None;
                }
                Err(err)
            }
        }
    }

    pub fn probe(&self, app: &AppHandle, project_id: &str) -> CodexProbe {
        let state = match state::load_state(app) {
            Ok(state) => state,
            Err(message) => return CodexProbe::fail(message),
        };
        let project = match state::find_project(&state, project_id) {
            Ok(project) => project.clone(),
            Err(message) => return CodexProbe::fail(message),
        };
        let binary = match resolve_codex(&state.settings) {
            Ok(path) => path,
            Err(message) => return CodexProbe::fail(message),
        };
        match self.with_server(&state.settings, |server| {
            let listed = server.list_threads(None)?;
            let matched = filter_threads_by_cwd(&listed, &project.path);
            Ok(CodexProbe {
                ok: true,
                binary: binary.display().to_string(),
                initialized: true,
                listed: listed.len(),
                cwd_matched: matched.len(),
                threads: matched,
                writer_safe: true,
                note: "已调用 thread/list，没有 thread/resume。审查走 review/start delivery=detached，不抢桌面 writer。".into(),
            })
        }) {
            Ok(probe) => probe,
            Err(message) => CodexProbe {
                ok: false,
                binary: binary.display().to_string(),
                initialized: false,
                listed: 0,
                cwd_matched: 0,
                threads: Vec::new(),
                writer_safe: true,
                note: message,
            },
        }
    }

    pub fn list_threads(&self, app: &AppHandle, project_id: &str) -> Result<CodexThreadList, String> {
        let state = state::load_state(app)?;
        let project = state::find_project(&state, project_id)?;
        let cwd = project.path.clone();
        self.with_server(&state.settings, |server| {
            let matched = server.list_threads(Some(&cwd))?;
            Ok(CodexThreadList {
                listed: matched.len(),
                cwd_matched: matched.len(),
                note: if matched.is_empty() {
                    "本机线程库里没有这个目录的 session。打开 Codex 桌面端在该项目聊过就会出现；也可以不勾选，提交审查时会新开专用审查线程。".into()
                } else {
                    String::new()
                },
                threads: matched,
            })
        })
    }

    pub fn git_snapshot(&self, app: &AppHandle, project_id: &str) -> Result<GitSnapshot, String> {
        let state = state::load_state(app)?;
        let project = state::find_project(&state, project_id)?;
        git_snapshot(&project.path)
    }

    pub fn start_review(&self, app: &AppHandle, req: ReviewRequest) -> Result<ReviewStartResult, String> {
        let state = state::load_state(app)?;
        let project = state::find_project(&state, &req.project_id)?;
        let cwd = project.path.clone();
        self.with_server(&state.settings, |server| server.start_reviews(&cwd, &req))
    }
}

struct AppServer {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
    next_id: i64,
}

impl AppServer {
    fn spawn(settings: &AppSettings) -> Result<Self, String> {
        let binary = resolve_codex(settings)?;
        let mut cmd = Command::new(&binary);
        cmd.arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        platform::prepare_command(&mut cmd);
        platform::apply_no_window(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|err| format!("无法启动 Codex App Server（{}）：{err}", binary.display()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Codex App Server 没有 stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Codex App Server 没有 stdout".to_string())?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        if tx.send(text).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            rx,
            next_id: 1,
        })
    }

    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn send(&mut self, value: Value) -> Result<(), String> {
        let mut line = serde_json::to_string(&value).map_err(|err| format!("编码 JSON-RPC 失败：{err}"))?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.flush())
            .map_err(|err| format!("写入 Codex App Server 失败：{err}"))
    }

    fn recv(&self, timeout: Duration) -> Result<Value, String> {
        let line = self
            .rx
            .recv_timeout(timeout)
            .map_err(|_| "Codex App Server 无响应".to_string())?;
        if line.trim().is_empty() {
            return self.recv(timeout);
        }
        serde_json::from_str(&line).map_err(|err| format!("Codex 返回了无法解析的行：{err}"))
    }

    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let (result, _notes) = self.request_collect(method, params, timeout, |_, _| true)?;
        Ok(result)
    }

    fn request_collect(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
        mut done: impl FnMut(&Value, &Value) -> bool,
    ) -> Result<(Value, Vec<Value>), String> {
        let id = self.next_id;
        self.next_id += 1;
        let mut message = json!({ "method": method, "id": id });
        if !params.is_null() {
            message["params"] = params;
        }
        self.send(message)?;
        let deadline = Instant::now() + timeout;
        let mut notes = Vec::new();
        let mut result = None;
        loop {
            let remain = deadline.saturating_duration_since(Instant::now());
            if remain.is_zero() {
                return Err(format!("等待 Codex `{method}` 超时"));
            }
            let value = self.recv(remain)?;
            if value.get("id") == Some(&json!(id)) {
                if let Some(error) = value.get("error") {
                    return Err(format_rpc_error(error));
                }
                result = Some(value.get("result").cloned().unwrap_or(Value::Null));
                let res = result.as_ref().unwrap();
                if done(res, &Value::Null) || notes.iter().any(|note| done(res, note)) {
                    break;
                }
                continue;
            }
            notes.push(value);
            if let Some(res) = result.as_ref() {
                if done(res, notes.last().unwrap()) {
                    break;
                }
            }
        }
        Ok((result.unwrap_or(Value::Null), notes))
    }

    fn handshake(&mut self) -> Result<(), String> {
        self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "agent-dock",
                    "title": "Agent Dock",
                    "version": "0.1.1"
                }
            }),
            INIT_TIMEOUT,
        )?;
        self.send(json!({ "method": "initialized", "params": {} }))
    }

    fn list_threads(&mut self, cwd: Option<&str>) -> Result<Vec<CodexThread>, String> {
        let kind_sets: [&[&str]; 3] = [
            &["cli", "vscode", "appServer", "exec", "unknown"],
            &["appServer"],
            &[],
        ];
        let mut by_id = std::collections::BTreeMap::new();
        for kinds in kind_sets {
            match self.list_page(cwd, kinds) {
                Ok(rows) => {
                    for row in rows {
                        by_id.entry(row.id.clone()).or_insert(row);
                    }
                }
                Err(err) if kinds.is_empty() => return Err(err),
                Err(_) => continue,
            }
        }
        if cwd.is_some() && by_id.is_empty() {
            if let Ok(all) = self.list_page(None, &["cli", "vscode", "appServer", "exec", "unknown"]) {
                for row in filter_threads_by_cwd(&all, cwd.unwrap()) {
                    by_id.entry(row.id.clone()).or_insert(row);
                }
            }
        }
        Ok(by_id.into_values().collect())
    }

    fn list_page(&mut self, cwd: Option<&str>, kinds: &[&str]) -> Result<Vec<CodexThread>, String> {
        let mut params = json!({
            "limit": 50,
            "sortKey": "updated_at"
        });
        if !kinds.is_empty() {
            params["sourceKinds"] = json!(kinds);
        }
        if let Some(cwd) = cwd {
            params["cwd"] = json!(cwd_variants(cwd));
        }
        let mut rows = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..6 {
            if let Some(next) = &cursor {
                params["cursor"] = json!(next);
            }
            let result = self.request("thread/list", params.clone(), LIST_TIMEOUT)?;
            for item in result.get("data").and_then(|v| v.as_array()).into_iter().flatten() {
                if let Some(thread) = parse_thread(item) {
                    rows.push(thread);
                }
            }
            cursor = result
                .get("nextCursor")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            if cursor.is_none() {
                break;
            }
        }
        Ok(rows)
    }

    fn start_reviews(&mut self, cwd: &str, req: &ReviewRequest) -> Result<ReviewStartResult, String> {
        let mut thread_ids = req.thread_ids.clone();
        thread_ids.retain(|id| !id.trim().is_empty());
        if thread_ids.is_empty() {
            let started = self.request(
                "thread/start",
                json!({
                    "cwd": cwd,
                    "approvalPolicy": "never",
                    "sandbox": "readOnly"
                }),
                LIST_TIMEOUT,
            );
            let started = match started {
                Ok(value) => value,
                Err(_) => self.request(
                    "thread/start",
                    json!({
                        "cwd": cwd,
                        "approvalPolicy": "never",
                        "sandbox": "workspaceWrite"
                    }),
                    LIST_TIMEOUT,
                )?,
            };
            let id = started
                .pointer("/thread/id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Codex 没有返回新线程 id".to_string())?;
            thread_ids.push(id.to_string());
        }

        let target = review_target(req);
        let mut reviews = Vec::new();
        for thread_id in thread_ids {
            reviews.push(self.review_one(&thread_id, &target)?);
        }
        let verdict = combine_verdicts(reviews.iter().map(|item| item.verdict.as_str()));
        Ok(ReviewStartResult { reviews, verdict })
    }

    fn review_one(&mut self, thread_id: &str, target: &Value) -> Result<BridgeReview, String> {
        let (result, notes) = self.request_collect(
            "review/start",
            json!({
                "threadId": thread_id,
                "delivery": "detached",
                "target": target
            }),
            REVIEW_TIMEOUT,
            |result, note| {
                is_review_done(note)
                    || matches!(
                        result.pointer("/turn/status").and_then(|v| v.as_str()),
                        Some("completed" | "failed" | "interrupted")
                    )
            },
        )?;
        let review_thread_id = result
            .get("reviewThreadId")
            .and_then(|v| v.as_str())
            .unwrap_or(thread_id)
            .to_string();
        let text = extract_review_text(&notes, &result);
        let verdict = parse_review_verdict(&text);
        Ok(BridgeReview {
            source_thread_id: thread_id.to_string(),
            review_thread_id,
            text,
            verdict,
        })
    }
}

impl Drop for AppServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThread {
    pub id: String,
    pub name: String,
    pub preview: String,
    pub cwd: Option<String>,
    pub source_kind: Option<String>,
    pub is_pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub model_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProbe {
    pub ok: bool,
    pub binary: String,
    pub initialized: bool,
    pub listed: usize,
    pub cwd_matched: usize,
    pub threads: Vec<CodexThread>,
    pub writer_safe: bool,
    pub note: String,
}

impl CodexProbe {
    fn fail(note: String) -> Self {
        Self {
            ok: false,
            binary: String::new(),
            initialized: false,
            listed: 0,
            cwd_matched: 0,
            threads: Vec::new(),
            writer_safe: true,
            note,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadList {
    pub threads: Vec<CodexThread>,
    pub listed: usize,
    pub cwd_matched: usize,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitSnapshot {
    pub branch: String,
    pub head: String,
    pub dirty: bool,
    pub summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
    pub project_id: String,
    pub thread_ids: Vec<String>,
    pub target_kind: String,
    pub commit_sha: Option<String>,
    pub base_branch: Option<String>,
    pub custom_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeReview {
    pub source_thread_id: String,
    pub review_thread_id: String,
    pub text: String,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartResult {
    pub reviews: Vec<BridgeReview>,
    pub verdict: String,
}

pub fn resolve_codex(settings: &AppSettings) -> Result<PathBuf, String> {
    let override_path = settings.codex_path.trim();
    if !override_path.is_empty() {
        let path = PathBuf::from(override_path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!("设置里的 Codex 路径不存在：{override_path}"));
    }
    platform::which_cmd("codex")
        .or_else(|| platform::which_cmd("codex.cmd"))
        .ok_or_else(|| {
            "找不到 Codex CLI。请安装并登录 Codex 桌面端/CLI，或在设置里填写 `codex` 路径。编排不会去点桌面窗口。"
                .into()
        })
}

pub fn cwd_variants(path: &str) -> Vec<String> {
    let raw = path.trim().trim_matches('"').to_string();
    let forward = raw.replace('\\', "/");
    let back = raw.replace('/', "\\");
    let mut out = vec![raw.clone(), forward.clone(), back.clone()];
    if let Some(rest) = forward.get(1..) {
        if forward.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && rest.starts_with(':') {
            out.push(format!("{}{rest}", forward.chars().next().unwrap().to_ascii_lowercase()));
            out.push(format!("{}{rest}", forward.chars().next().unwrap().to_ascii_uppercase()));
        }
    }
    out.sort();
    out.dedup();
    out
}

fn filter_threads_by_cwd(threads: &[CodexThread], cwd: &str) -> Vec<CodexThread> {
    threads
        .iter()
        .filter(|thread| thread_matches_cwd(thread, cwd))
        .cloned()
        .collect()
}

fn thread_matches_cwd(thread: &CodexThread, cwd: &str) -> bool {
    match &thread.cwd {
        Some(thread_cwd) if !thread_cwd.is_empty() => path_norm::paths_equal(thread_cwd, cwd),
        _ => false,
    }
}

fn parse_thread(value: &Value) -> Option<CodexThread> {
    let id = value.get("id")?.as_str()?.to_string();
    let preview = json_str(value, &["preview", "name"]).unwrap_or_default();
    let name = json_str(value, &["name", "preview"]).unwrap_or_else(|| id.clone());
    Some(CodexThread {
        id,
        name,
        preview,
        cwd: json_str(value, &["cwd"]).or_else(|| {
            value
                .pointer("/gitInfo/cwd")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }),
        source_kind: json_str(value, &["sourceKind", "source"]),
        is_pinned: value.get("isPinned").and_then(|v| v.as_bool()).unwrap_or(false),
        created_at: json_ts(value, &["createdAt", "created_at"]),
        updated_at: json_ts(value, &["updatedAt", "updated_at", "createdAt"]),
        model_provider: json_str(value, &["modelProvider"]),
    })
}

fn json_str(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(|v| v.as_str()) {
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn json_ts(value: &Value, keys: &[&str]) -> i64 {
    for key in keys {
        if let Some(n) = value.get(*key).and_then(|v| v.as_i64()) {
            return n;
        }
        if let Some(n) = value.get(*key).and_then(|v| v.as_f64()) {
            return n as i64;
        }
    }
    0
}

fn review_target(req: &ReviewRequest) -> Value {
    let ticket = "\n\n结束时单独一行写 VERDICT: PASS 或 VERDICT: FAIL。";
    match req.target_kind.as_str() {
        "commit" => json!({
            "type": "commit",
            "sha": req.commit_sha.clone().unwrap_or_default()
        }),
        "baseBranch" => json!({
            "type": "baseBranch",
            "branch": req.base_branch.clone().unwrap_or_else(|| "main".into())
        }),
        "custom" => json!({
            "type": "custom",
            "instructions": format!(
                "{}{ticket}",
                req.custom_instructions
                    .as_deref()
                    .unwrap_or("Review the current working tree.")
            )
        }),
        _ => json!({ "type": "uncommittedChanges" }),
    }
}

fn is_review_done(note: &Value) -> bool {
    let method = note.get("method").and_then(|v| v.as_str()).unwrap_or("");
    if method == "turn/completed" {
        return true;
    }
    method == "item/completed" && item_type(note).as_deref() == Some("exitedReviewMode")
}

fn item_type(note: &Value) -> Option<String> {
    note.pointer("/params/item/type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_review_text(notes: &[Value], result: &Value) -> String {
    for note in notes.iter().rev() {
        if let Some(text) = note.pointer("/params/item/review").and_then(|v| v.as_str()) {
            if item_type(note).as_deref() == Some("exitedReviewMode") && !text.is_empty() {
                return text.to_string();
            }
        }
        if let Some(text) = note.pointer("/params/item/text").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                return text.to_string();
            }
        }
        if let Some(text) = note
            .pointer("/params/item/content")
            .and_then(|v| v.as_array())
            .and_then(|items| {
                items.iter().find_map(|item| {
                    item.get("text")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                })
            })
        {
            return text.to_string();
        }
    }
    result
        .pointer("/turn/items")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items.iter().rev().find_map(|item| {
                item.get("review")
                    .or_else(|| item.get("text"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
        })
        .unwrap_or_default()
}

pub fn parse_review_verdict(text: &str) -> String {
    let sample = window_text(text);
    if sample.is_empty() {
        return "unknown".into();
    }
    let fail = regex_contains_fail(&sample);
    let pass = regex_contains_pass(&sample);
    if fail {
        "fail".into()
    } else if pass {
        "pass".into()
    } else {
        "unknown".into()
    }
}

fn window_text(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let mut out: Vec<&str> = Vec::new();
    out.extend(lines.iter().take(10));
    if lines.len() > 10 {
        out.extend(lines.iter().rev().take(10).rev());
    }
    out.join("\n")
}

fn regex_contains_fail(sample: &str) -> bool {
    contains_word(sample, &["FAIL", "FAILED", "VERDICT: FAIL", "VERDICT：FAIL", "REVIEW: FAIL"])
        || sample.contains("未通过")
}

fn regex_contains_pass(sample: &str) -> bool {
    contains_word(sample, &["PASS", "PASSED", "VERDICT: PASS", "VERDICT：PASS", "REVIEW: PASS"])
        || (sample.contains("通过") && !sample.contains("未通过"))
}

fn contains_word(sample: &str, needles: &[&str]) -> bool {
    let upper = sample.to_ascii_uppercase();
    needles.iter().any(|needle| upper.contains(&needle.to_ascii_uppercase()))
}

fn combine_verdicts<'a>(verdicts: impl Iterator<Item = &'a str>) -> String {
    let items: Vec<&str> = verdicts.collect();
    if items.is_empty() {
        return "unknown".into();
    }
    if items.iter().any(|item| *item == "fail") {
        return "fail".into();
    }
    if items.iter().all(|item| *item == "pass") {
        return "pass".into();
    }
    "unknown".into()
}

fn format_rpc_error(error: &Value) -> String {
    let message = error
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("Codex App Server 返回错误");
    let lower = message.to_ascii_lowercase();
    if lower.contains("active writer") || lower.contains("already has an active writer") {
        return "这条 thread 正在被桌面端占用（同一时刻只能有一个 writer）。请改用 detached 审查，或先在桌面端结束当前 turn。".into();
    }
    if let Some(code) = error.get("code") {
        return format!("Codex 错误 {code}：{message}");
    }
    message.to_string()
}

fn git_cmd(cwd: &str, args: &[&str]) -> Result<String, String> {
    if !Path::new(cwd).is_dir() {
        return Err(format!("项目文件夹读不了：{cwd}"));
    }
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    platform::prepare_command(&mut cmd);
    platform::apply_no_window(&mut cmd);
    let output = cmd
        .output()
        .map_err(|err| format!("无法执行 git：{err}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_snapshot(cwd: &str) -> Result<GitSnapshot, String> {
    let branch = git_cmd(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|_| "—".into());
    let head = git_cmd(cwd, &["rev-parse", "HEAD"]).unwrap_or_default();
    let porcelain = git_cmd(cwd, &["status", "--porcelain"]).unwrap_or_default();
    let stat = git_cmd(cwd, &["diff", "--stat"]).unwrap_or_default();
    let summary = if porcelain.is_empty() && stat.is_empty() {
        String::new()
    } else {
        [porcelain, stat]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(GitSnapshot {
        dirty: !summary.is_empty(),
        summary,
        branch,
        head,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_omits_jsonrpc() {
        let message = json!({ "method": "initialize", "id": 1, "params": {} });
        let text = serde_json::to_string(&message).unwrap();
        assert!(!text.contains("jsonrpc"));
        assert!(text.contains("initialize"));
    }

    #[test]
    fn cwd_variants_cover_slash_styles() {
        let variants = cwd_variants(r"D:\idea_jidian_projects\aitools");
        assert!(variants.iter().any(|item| item.contains('/')));
        assert!(variants.iter().any(|item| item.contains('\\')));
    }

    #[test]
    fn parse_verdict_reads_tickets() {
        assert_eq!(parse_review_verdict("Looks good.\nVERDICT: PASS"), "pass");
        assert_eq!(parse_review_verdict("缺测试\nVERDICT: FAIL"), "fail");
        assert_eq!(parse_review_verdict("闸门：未通过"), "fail");
        assert_eq!(parse_review_verdict("Looks solid overall."), "unknown");
    }

    #[test]
    fn thread_cwd_filter_uses_path_norm() {
        let thread = CodexThread {
            id: "thr_1".into(),
            name: "demo".into(),
            preview: String::new(),
            cwd: Some("D:/idea_jidian_projects/aitools/".into()),
            source_kind: Some("appServer".into()),
            is_pinned: false,
            created_at: 0,
            updated_at: 0,
            model_provider: None,
        };
        assert!(thread_matches_cwd(&thread, r"D:\idea_jidian_projects\aitools"));
        assert!(!thread_matches_cwd(&thread, r"D:\other"));
    }

    #[test]
    fn combine_requires_all_pass() {
        assert_eq!(combine_verdicts(["pass", "pass"].into_iter()), "pass");
        assert_eq!(combine_verdicts(["pass", "unknown"].into_iter()), "unknown");
        assert_eq!(combine_verdicts(["pass", "fail"].into_iter()), "fail");
    }

    #[test]
    fn review_start_payload_is_detached() {
        let payload = json!({
            "threadId": "thr_123",
            "delivery": "detached",
            "target": { "type": "uncommittedChanges" }
        });
        assert_eq!(payload["delivery"], "detached");
        assert_ne!(payload["delivery"], "inline");
    }
}
