//! Native OMP process registry. IPC callers never own process lifetime.
use crate::{
    platform,
    pty::{LivePtyInfo, PtyOpened},
    state::{self, AppState},
    tools::{self, omp},
};
use omp_rpc::{Client, ClientOptions, Event, RpcInbound};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub phase: &'static str,
    pub started_at: Option<i64>,
    pub last_progress_at: Option<i64>,
}
impl Default for Activity {
    fn default() -> Self {
        Self {
            phase: "idle",
            started_at: None,
            last_progress_at: None,
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub runtime_id: String,
    pub seq: u64,
    pub info: LivePtyInfo,
    pub profile_id: String,
    pub setup_required: bool,
    pub activity: Activity,
    pub state: Value,
    pub messages: Vec<Value>,
    pub tools: HashMap<String, Value>,
    pub dialogs: Vec<Value>,
    pub subagents: HashMap<String, Value>,
    pub error: Option<String>,
    pub notifications: Vec<Value>,
    pub settled: bool,
    pub prompt_results: HashMap<String, Value>,
}
struct Session {
    client: Arc<Client>,
    snapshot: Mutex<Snapshot>,
}
pub struct Hub {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    opening: Mutex<()>,
}
static HUB: OnceLock<Arc<Hub>> = OnceLock::new();
pub fn global() -> Arc<Hub> {
    HUB.get_or_init(|| {
        Arc::new(Hub {
            sessions: Mutex::new(HashMap::new()),
            opening: Mutex::new(()),
        })
    })
    .clone()
}
pub fn rename_running(id: &str, title: &str) -> Result<bool, String> {
    let hub = global();
    let live = hub.list();
    if let Some(info) = live.iter().find(|s| s.session_id.as_deref() == Some(id)) {
        hub.request(
            &info.pty_id,
            json!({"type":"set_session_name","name":title}),
        )?;
        return Ok(true);
    }
    Ok(false)
}
fn fail(e: impl std::fmt::Display) -> String {
    e.to_string()
}
impl Hub {
    fn session(&self, id: &str) -> Result<Arc<Session>, String> {
        self.sessions
            .lock()
            .map_err(fail)?
            .get(id)
            .cloned()
            .ok_or("OMP 会话已关闭，请重新打开".into())
    }
    pub fn list(&self) -> Vec<LivePtyInfo> {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| !s.client.is_closed())
            .map(|s| s.snapshot.lock().unwrap().info.clone())
            .collect()
    }
    pub fn snapshot(&self, id: &str) -> Result<Snapshot, String> {
        Ok(self.session(id)?.snapshot.lock().map_err(fail)?.clone())
    }
    pub fn open(
        self: &Arc<Self>,
        app: AppHandle,
        state: &AppState,
        project_id: &str,
        session_id: Option<String>,
        profile: Option<String>,
        title: String,
    ) -> Result<PtyOpened, String> {
        let _opening = self.opening.lock().map_err(fail)?;
        let project = state::find_project(state, project_id)?;
        let profile = if let Some(id) = &session_id {
            omp::split_id(id)?.0
        } else {
            profile
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| omp::default_profile(&state.settings))
        };
        omp::validate_profile(&profile)?;
        if let Some(id) = &session_id {
            if let Some(live) = self
                .list()
                .iter()
                .find(|l| l.project_id == project_id && l.session_id.as_ref() == Some(id))
            {
                return Ok(opened(live, true));
            }
        }
        let (client, events, setup_required) = crate::omp_startup::connect(
            |_| {
                let mut cmd = omp::command(&state.settings, &profile)?;
                cmd.current_dir(&project.path).args(["--mode", "rpc-ui"]);
                if let Some(id) = &session_id {
                    cmd.arg("--resume").arg(
                        omp::find_session_file(&project.path, id).ok_or("OMP 历史文件不存在")?,
                    );
                } else {
                    cmd.arg("--no-title");
                }
                cmd.envs(crate::proxy::proxy_env(
                    project.proxy_enabled,
                    &project.proxy_url,
                )?);
                if project.proxy_enabled {
                    cmd.env("PI_PROXY", &project.proxy_url);
                }
                Ok(cmd)
            },
            ClientOptions {
                defer_negotiation: true,
                ..Default::default()
            },
        )
        .map_err(fail)?;
        let id = format!("omp-{}", Uuid::new_v4());
        let info = LivePtyInfo {
            pty_id: id.clone(),
            key: format!("{project_id}:omp:{}", session_id.as_deref().unwrap_or(&id)),
            project_id: project_id.into(),
            tool_id: tools::ToolId::Omp,
            session_id,
            title,
            alive: true,
            opened_at: chrono::Utc::now().timestamp_millis(),
            kind: "ompRpc".into(),
            url: None,
            account: Some(tools::SessionAccount {
                id: profile.clone(),
                name: profile.clone(),
            }),
        };
        let session = Arc::new(Session {
            client: Arc::new(client),
            snapshot: Mutex::new(Snapshot {
                runtime_id: id.clone(),
                seq: 0,
                info: info.clone(),
                profile_id: profile,
                setup_required,
                activity: Activity::default(),
                state: json!({}),
                messages: vec![],
                tools: HashMap::new(),
                dialogs: vec![],
                subagents: HashMap::new(),
                notifications: vec![],
                error: None,
                settled: true,
                prompt_results: HashMap::new(),
            }),
        });
        self.sessions
            .lock()
            .map_err(fail)?
            .insert(id.clone(), session.clone());
        let event_session = Arc::downgrade(&session);
        let event_app = app.clone();
        let event_id = id.clone();
        std::thread::spawn(move || {
            for event in events {
                let value = match event {
                    Event::Notification(v) => serde_json::to_value(v).unwrap_or(Value::Null),
                    Event::Undecodable { raw, .. } => raw,
                    Event::UnmatchedResponse(v) => serde_json::to_value(v).unwrap_or(Value::Null),
                };
                let Some(s) = event_session.upgrade() else {
                    break;
                };
                let mut snap = s.snapshot.lock().unwrap();
                reduce(&mut snap, &value);
                let seq = snap.seq;
                drop(snap);
                let _ = event_app.emit(
                    "omp-event",
                    json!({"runtimeId":event_id,"seq":seq,"event":value}),
                );
            }
            if let Some(s) = event_session.upgrade() {
                let mut snap = s.snapshot.lock().unwrap();
                snap.info.alive = false;
                let detail = s.client.stderr_tail();
                snap.error = Some(if detail.is_empty() {
                    "OMP 进程已退出，可重新打开已保存的会话".into()
                } else {
                    format!("OMP 进程已退出：\n{detail}")
                });
                snap.seq += 1;
                let _ = event_app.emit(
                    "omp-event",
                    json!({"runtimeId":event_id,"seq":snap.seq,"event":{"type":"disconnected"}}),
                );
            }
        });
        let init_hub = self.clone();
        let init_id = id.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let ready = session.client.ready();
                if ready
                    .supported_protocol_versions
                    .as_ref()
                    .is_some_and(|v| v.contains(&2))
                {
                    init_hub.request(
                        &init_id,
                        json!({"type":"negotiate_protocol","protocolVersion":2}),
                    )?;
                }
                init_hub.request(&init_id, json!({"type":"set_ask_dialog","enabled":true}))?;
                init_hub.request(
                    &init_id,
                    json!({"type":"set_subagent_subscription","level":"events"}),
                )?;
                if let Ok(response) = init_hub.request(&init_id, json!({"type":"get_subagents"})) {
                    if let Some(agents) = response["data"]["subagents"].as_array() {
                        let mut snap = session.snapshot.lock().unwrap();
                        for agent in agents {
                            if let Some(id) = agent["id"].as_str() {
                                snap.subagents.insert(id.into(), agent.clone());
                            }
                        }
                    }
                }
                init_hub.refresh(&init_id)?;
                Ok(())
            })();
            if let Err(e) = result {
                session.snapshot.lock().unwrap().error = Some(e);
            }
            let _ = app.emit(
                "omp-event",
                json!({"runtimeId":init_id,"event":{"type":"snapshot"}}),
            );
        });
        Ok(opened(&info, false))
    }
    pub fn refresh(&self, id: &str) -> Result<(), String> {
        let s = self.session(id)?;
        let before_seq = s.snapshot.lock().map_err(fail)?.seq;
        let state = self.request(id, json!({"type":"get_state"}))?["data"].clone();
        // Page history through the negotiated transport; discard partial walks on a stale cursor.
        let mut cursor = Value::Null;
        let mut messages = Vec::new();
        loop {
            let mut cmd = json!({"type":"get_messages_page","limit":100});
            if !cursor.is_null() {
                cmd["cursor"] = cursor;
            }
            let response = self.request(id, cmd);
            let page = match response {
                Ok(v) => v["data"].clone(),
                Err(_) => {
                    messages = self.request(id, json!({"type":"get_messages"}))?["data"]
                        ["messages"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    break;
                }
            };
            messages.extend(page["messages"].as_array().cloned().unwrap_or_default());
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
        }
        let mut snap = s.snapshot.lock().map_err(fail)?;
        apply_refresh(&mut snap, state, messages, before_seq);
        Ok(())
    }
    pub fn request(&self, id: &str, frame: Value) -> Result<Value, String> {
        let kind = frame["type"].as_str().ok_or("OMP 命令缺少 type")?;
        const ALLOWED: &[&str] = &[
            "negotiate_protocol",
            "prompt",
            "abort",
            "steer",
            "follow_up",
            "remove_queued_message",
            "promote_queued_message",
            "abort_and_prompt",
            "get_state",
            "get_messages",
            "get_messages_page",
            "get_available_models",
            "set_model",
            "get_available_thinking_levels",
            "set_thinking_level",
            "set_ask_dialog",
            "get_available_commands",
            "set_subagent_subscription",
            "get_subagents",
            "get_subagent_messages",
            "cancel_subagent",
            "steer_subagent",
            "get_tree",
            "get_entries",
            "navigate_tree",
            "branch",
            "fork",
            "compact",
            "get_session_stats",
            "set_session_name",
            "get_login_providers",
            "login",
            "get_logout_accounts",
            "logout",
            "abort_branch_summary",
        ];
        if !ALLOWED.contains(&kind) {
            return Err(format!("不支持的 OMP 命令：{kind}"));
        }
        let s = self.session(id)?;
        let (before_settled, before_activity, before_seq) = {
            let mut snap = s.snapshot.lock().map_err(fail)?;
            if kind == "prompt" && snap.setup_required {
                return Err("此 OMP profile 尚未配置可用模型，请先登录并选择模型".into());
            }
            let before = (snap.settled, snap.activity.clone(), snap.seq);
            if kind == "prompt" && snap.settled {
                snap.settled = false;
                snap.activity = Activity {
                    phase: "working",
                    started_at: Some(chrono::Utc::now().timestamp_millis()),
                    last_progress_at: None,
                };
            }
            before
        };
        let response = match s.client.request_value(
            frame.clone(),
            Duration::from_secs(if matches!(kind, "login" | "compact" | "navigate_tree") {
                600
            } else {
                30
            }),
        ) {
            Ok(response) => response,
            Err(error) => {
                // An explicit rejection without a run must not leave a phantom busy state.
                if kind == "prompt" && matches!(&error, omp_rpc::Error::Command { .. }) {
                    let mut snap = s.snapshot.lock().map_err(fail)?;
                    if snap.seq == before_seq {
                        snap.settled = before_settled;
                        snap.activity = before_activity;
                    }
                }
                return Err(error.to_string());
            }
        };
        if kind == "prompt"
            && response
                .data
                .as_ref()
                .and_then(|v| v["agentInvoked"].as_bool())
                == Some(false)
        {
            let mut snap = s.snapshot.lock().map_err(fail)?;
            snap.settled = before_settled;
            snap.activity = before_activity;
        }
        if matches!(kind, "fork" | "branch" | "navigate_tree")
            && !response
                .data
                .as_ref()
                .is_some_and(|v| v["cancelled"] == true)
        {
            self.refresh(id)?;
        }
        if kind == "set_session_name" {
            s.snapshot.lock().unwrap().info.title = frame["name"].as_str().unwrap_or("OMP").into();
        }
        if kind == "set_model" {
            s.snapshot.lock().map_err(fail)?.setup_required = false;
        }
        serde_json::to_value(response).map_err(fail)
    }
    pub fn respond(&self, id: &str, response: Value) -> Result<(), String> {
        let request_id = response["id"].clone();
        let frame: RpcInbound = serde_json::from_value(response).map_err(fail)?;
        if !matches!(frame, RpcInbound::ExtensionUiResponse(_)) {
            return Err("仅接受 OMP 交互响应".into());
        }
        let session = self.session(id)?;
        session.client.send(&frame).map_err(fail)?;
        let mut snap = session.snapshot.lock().map_err(fail)?;
        snap.dialogs.retain(|d| d["id"] != request_id);
        snap.seq += 1;
        Ok(())
    }
    pub fn close(&self, id: &str) -> Result<(), String> {
        let s = self
            .sessions
            .lock()
            .map_err(fail)?
            .remove(id)
            .ok_or("OMP 已关闭")?;
        s.client.disconnect();
        #[cfg(windows)]
        if let Some(pid) = s.client.pid() {
            let mut c = std::process::Command::new("taskkill.exe");
            c.args(["/PID", &pid.to_string(), "/T", "/F"]);
            platform::apply_no_window(&mut c);
            let _ = c.output();
        }
        Ok(())
    }
    pub fn close_all(&self) {
        for id in self
            .list()
            .iter()
            .map(|s| s.pty_id.clone())
            .collect::<Vec<_>>()
        {
            let _ = self.close(&id);
        }
    }
}
fn apply_refresh(snap: &mut Snapshot, state: Value, messages: Vec<Value>, before_seq: u64) {
    let raced = snap.seq != before_seq;
    let recent_state = snap.state.clone();
    let streaming = state["isStreaming"] == true;
    let settled = state["isSettled"].as_bool().unwrap_or(!streaming);
    snap.state = state.clone();
    // OMP's history omits the in-flight partial. A quiet thinking stream is
    // still active even when no new event raced this read.
    if !raced && settled || snap.messages.is_empty() {
        snap.messages = messages;
    }
    if !raced {
        snap.settled = settled;
        if settled {
            snap.activity.phase = "idle";
        } else if !streaming && state["hasPendingAsyncWork"] == true {
            snap.activity.phase = "background";
        } else if snap.activity.phase == "idle" {
            snap.activity.phase = "working";
        }
    }
    snap.state["queues"] = state["queuedMessages"].clone();
    if raced {
        for key in ["model", "thinkingLevel", "queues"] {
            if let Some(value) = recent_state.get(key) {
                snap.state[key] = value.clone();
            }
        }
    }
    if let Some(id) = state["sessionId"].as_str() {
        snap.info.session_id = Some(omp::public_id(&snap.profile_id, id));
    }
    if let Some(title) = state["sessionName"].as_str().filter(|s| !s.is_empty()) {
        snap.info.title = title.into();
    }
    snap.seq += 1;
}
fn opened(info: &LivePtyInfo, reused: bool) -> PtyOpened {
    PtyOpened {
        pty_id: info.pty_id.clone(),
        key: info.key.clone(),
        reused,
        session_id: info.session_id.clone(),
        title: info.title.clone(),
        opened_at: info.opened_at,
        kind: info.kind.clone(),
        url: None,
        account: info.account.clone(),
    }
}
fn reduce(s: &mut Snapshot, e: &Value) {
    s.seq += 1;
    let now = chrono::Utc::now().timestamp_millis();
    match e["type"].as_str().unwrap_or("") {
        "agent_start" => {
            if s.settled || s.activity.started_at.is_none() {
                s.activity.started_at = Some(now);
            }
            s.activity.phase = "working";
            s.activity.last_progress_at = Some(now);
        }
        "message_update" => {
            s.activity.last_progress_at = Some(now);
            s.activity.phase = match e["assistantMessageEvent"]["type"].as_str().unwrap_or("") {
                kind if kind.starts_with("thinking_") => "thinking",
                kind if kind.starts_with("text_") => "responding",
                kind if kind.starts_with("toolcall_") => "tool",
                _ => s.activity.phase,
            };
        }
        "tool_execution_start" | "tool_execution_update" | "tool_stream_update" => {
            s.activity.phase = "tool";
            s.activity.last_progress_at = Some(now);
        }
        "tool_execution_end" | "auto_retry_end" => {
            s.activity.phase = "working";
            s.activity.last_progress_at = Some(now);
        }
        "auto_retry_start" => {
            s.activity.phase = "retrying";
            s.activity.last_progress_at = Some(now);
        }
        "subagent_lifecycle" | "subagent_progress" | "subagent_event" => {
            s.activity.last_progress_at = Some(now);
        }
        "prompt_result" => {
            s.activity.phase = if e["sessionSettled"] == true {
                "idle"
            } else {
                "background"
            }
        }
        "session_settled" => s.activity.phase = "idle",
        _ => {}
    }
    match e["type"].as_str().unwrap_or("") {
        "agent_start" => {
            s.settled = false;
            s.error = None;
        }
        "session_settled" => s.settled = true,
        "prompt_result" => {
            if let Some(id) = e["id"].as_str() {
                s.prompt_results.insert(id.into(), e.clone());
            }
            if e["sessionSettled"] == true {
                s.settled = true;
            }
            if e["status"] == "error" {
                s.error = Some(
                    e["error"]["message"]
                        .as_str()
                        .unwrap_or("OMP 请求失败")
                        .into(),
                );
            }
        }
        "message_start" | "message_update" | "message_end" => {
            let mut m = e["message"].clone();
            if m.is_null() {
                return;
            }
            let id = e["messageId"]
                .as_str()
                .or_else(|| m["id"].as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("message-{}", s.messages.len()));
            m["_dockId"] = json!(id);
            if let Some(old) = s.messages.iter_mut().find(|m| m["_dockId"] == id) {
                *old = m;
            } else {
                s.messages.push(m);
            }
        }
        "tool_execution_start" | "tool_execution_update" | "tool_execution_end" => {
            if let Some(id) = e["toolCallId"].as_str() {
                let mut v = s.tools.get(id).cloned().unwrap_or(json!({}));
                if let (Some(old), Some(new)) = (v.as_object_mut(), e.as_object()) {
                    old.extend(new.clone());
                }
                s.tools.insert(id.into(), v);
            }
        }
        "extension_ui_request" => {
            if e["method"] == "cancel" {
                s.dialogs.retain(|d| d["id"] != e["targetId"]);
            } else if matches!(
                e["method"].as_str(),
                Some("select" | "confirm" | "input" | "editor" | "ask")
            ) {
                s.dialogs.push(e.clone());
            } else if matches!(
                e["method"].as_str(),
                Some(
                    "notify"
                        | "setStatus"
                        | "setWidget"
                        | "setTitle"
                        | "set_editor_text"
                        | "open_url"
                )
            ) {
                s.notifications.push(e.clone());
                if s.notifications.len() > 30 {
                    s.notifications.remove(0);
                }
            } else {
                let mut dialog = e.clone();
                dialog["unsupported"] = json!(true);
                s.dialogs.push(dialog);
            }
        }
        "subagent_lifecycle" | "subagent_progress" | "subagent_event" => {
            let p = e.get("payload").unwrap_or(e);
            let key = p["id"]
                .as_str()
                .map(str::to_string)
                .or_else(|| {
                    s.subagents
                        .iter()
                        .find(|(_, a)| a["index"] == p["index"] && a["agent"] == p["agent"])
                        .map(|(id, _)| id.clone())
                })
                .unwrap_or_else(|| format!("agent-{}", p["index"]));
            let mut agent = s.subagents.get(&key).cloned().unwrap_or(json!({}));
            if let (Some(old), Some(new)) = (agent.as_object_mut(), p.as_object()) {
                old.extend(new.clone());
            }
            s.subagents.insert(key, agent);
        }
        "queue_update" => {
            s.state["queues"] = e.get("queuedMessages").unwrap_or(e).clone();
        }
        "model_changed" | "config_update" => {
            if e["model"].is_object() {
                s.state["model"] = e["model"].clone();
            }
            if e["thinkingLevel"].is_string() {
                s.state["thinkingLevel"] = e["thinkingLevel"].clone();
            }
        }
        "thinking_level_changed" => {
            s.state["thinkingLevel"] = e
                .get("thinkingLevel")
                .or_else(|| e.get("level"))
                .cloned()
                .unwrap_or(Value::Null);
        }
        "session_info_update" => {
            if let Some(id) = e["sessionId"].as_str() {
                let id = omp::public_id(&s.profile_id, id);
                if s.info.session_id.as_ref() != Some(&id) {
                    s.messages.clear();
                    s.tools.clear();
                    s.prompt_results.clear();
                    s.dialogs.clear();
                }
                s.info.key = format!("{}:omp:{id}", s.info.project_id);
                s.info.session_id = Some(id);
            }
            if let Some(title) = e["title"].as_str() {
                s.info.title = title.into();
            }
        }
        "command_output" => {
            s.notifications.push(e.clone());
            if s.notifications.len() > 30 {
                s.notifications.remove(0);
            }
        }
        "notice" if e["level"] == "error" => {
            s.error = Some(e["message"].as_str().unwrap_or("OMP 错误").into());
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> Snapshot {
        Snapshot {
            runtime_id: "omp-test".into(),
            seq: 0,
            profile_id: "work".into(),
            setup_required: false,
            activity: Activity::default(),
            state: json!({}),
            messages: vec![],
            tools: HashMap::new(),
            dialogs: vec![],
            subagents: HashMap::new(),
            notifications: vec![],
            error: None,
            settled: true,
            prompt_results: HashMap::new(),
            info: LivePtyInfo {
                account: None,
                pty_id: "omp-test".into(),
                key: "key".into(),
                project_id: "p".into(),
                tool_id: tools::ToolId::Omp,
                session_id: None,
                title: "test".into(),
                alive: true,
                opened_at: 0,
                kind: "ompRpc".into(),
                url: None,
            },
        }
    }
    #[test]
    fn a_yield_with_background_work_does_not_finish_the_session() {
        let mut s = snapshot();
        reduce(&mut s, &json!({"type":"agent_start"}));
        reduce(
            &mut s,
            &json!({"type":"prompt_result","id":"own","status":"completed","sessionSettled":false}),
        );
        assert!(!s.settled);
        assert_eq!(s.activity.phase, "background");
        assert_eq!(s.prompt_results["own"]["status"], "completed");
        reduce(&mut s, &json!({"type":"session_settled"}));
        assert!(s.settled);
    }
    #[test]
    fn a_quiet_stream_keeps_its_partial_during_status_sync() {
        let mut s = snapshot();
        reduce(&mut s, &json!({"type":"agent_start"}));
        reduce(
            &mut s,
            &json!({"type":"message_update","messageId":"thinking","message":{"role":"assistant","content":[{"type":"thinking","thinking":"The"}]},"assistantMessageEvent":{"type":"thinking_delta","delta":"The"}}),
        );
        let seq = s.seq;
        apply_refresh(
            &mut s,
            json!({"isStreaming":true,"isSettled":false}),
            vec![],
            seq,
        );
        assert_eq!(s.messages[0]["content"][0]["thinking"], "The");
        assert_eq!(s.activity.phase, "thinking");
        assert!(!s.settled);
        assert!(s.activity.last_progress_at.is_some());
        let seq = s.seq;
        apply_refresh(
            &mut s,
            json!({"isStreaming":false,"isSettled":true}),
            vec![json!({"role":"assistant","content":"Finished"})],
            seq,
        );
        assert!(s.settled);
        assert_eq!(s.activity.phase, "idle");
        assert_eq!(s.messages[0]["content"], "Finished");
    }
    #[test]
    fn status_sync_does_not_end_background_agents_or_overwrite_a_new_event() {
        let mut s = snapshot();
        reduce(&mut s, &json!({"type":"agent_start"}));
        let seq = s.seq;
        apply_refresh(
            &mut s,
            json!({"isStreaming":false,"isSettled":false,"hasPendingAsyncWork":true}),
            vec![],
            seq,
        );
        assert!(!s.settled);
        assert_eq!(s.activity.phase, "background");
        let seq = s.seq;
        reduce(&mut s, &json!({"type":"session_settled"}));
        apply_refresh(&mut s, json!({"isSettled":false}), vec![], seq);
        assert!(s.settled);
    }
    #[test]
    fn stream_updates_replace_the_same_message() {
        let mut s = snapshot();
        for kind in ["message_start", "message_update", "message_end"] {
            reduce(
                &mut s,
                &json!({"type":kind,"messageId":"m","message":{"role":"assistant","content":[{"type":"text","text":kind}]}}),
            );
        }
        assert_eq!(s.messages.len(), 1);
        assert_eq!(s.messages[0]["content"][0]["text"], "message_end");
    }
    #[test]
    fn cancel_removes_only_the_target_dialog() {
        let mut s = snapshot();
        for id in ["one", "two"] {
            reduce(
                &mut s,
                &json!({"type":"extension_ui_request","id":id,"method":"confirm"}),
            );
        }
        reduce(
            &mut s,
            &json!({"type":"extension_ui_request","method":"cancel","targetId":"one"}),
        );
        assert_eq!(s.dialogs.len(), 1);
        assert_eq!(s.dialogs[0]["id"], "two");
    }
    #[test]
    fn subagents_use_official_payload_identity_and_merge_progress() {
        let mut s = snapshot();
        reduce(
            &mut s,
            &json!({"type":"subagent_lifecycle","payload":{"id":"worker","index":0,"agent":"reviewer","status":"running"}}),
        );
        reduce(
            &mut s,
            &json!({"type":"subagent_progress","payload":{"index":0,"agent":"reviewer","task":"check","progress":{"text":"reading"}}}),
        );
        assert_eq!(s.subagents.len(), 1);
        assert_eq!(s.subagents["worker"]["status"], "running");
        assert_eq!(s.subagents["worker"]["progress"]["text"], "reading");
    }
}
