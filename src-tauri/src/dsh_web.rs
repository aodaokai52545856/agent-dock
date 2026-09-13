use crate::dsh_keys;
use crate::dsh_web_guard::{self, PidRecord};
use crate::platform;
use crate::proxy::proxy_env;
use crate::pty::{LivePtyInfo, PtyOpened};
use crate::state::{AppState, find_project};
use crate::tools::{self, ToolId};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const URL_WAIT: Duration = Duration::from_secs(90);
const OWNER_ENV: &str = "AD_DSH_OWNER";

struct WebSession {
    id: String,
    key: String,
    project_id: String,
    session_id: Option<String>,
    title: String,
    url: String,
    opened_at: i64,
    child: Child,
}

pub struct Hub {
    sessions: Mutex<HashMap<String, WebSession>>,
    shutting_down: AtomicBool,
    lock_path: Mutex<Option<PathBuf>>,
    #[cfg(windows)]
    job: Mutex<Option<KillJob>>,
}

impl Hub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
            shutting_down: AtomicBool::new(false),
            lock_path: Mutex::new(None),
            #[cfg(windows)]
            job: Mutex::new(KillJob::new()),
        })
    }

    pub fn set_lock_path(&self, path: PathBuf) {
        if let Ok(mut guard) = self.lock_path.lock() {
            *guard = Some(path);
        }
    }

    pub fn list(&self) -> Vec<LivePtyInfo> {
        let mut sessions = lock_sessions(&self.sessions);
        reap(&mut sessions);
        sessions.values().map(live_info).collect()
    }

    pub fn reap_strays(&self) {
        let live = self.live_child_pids();
        let _ = reap_strays_with(&live, self.lock_records(), |pid| self.forget_lock_pid(pid));
    }

    pub fn shutdown(&self) {
        if self.shutting_down.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut sessions = lock_sessions(&self.sessions);
        for (_, mut session) in sessions.drain() {
            let _ = kill_tree(&mut session.child);
        }
        drop(sessions);
        self.write_lock_records(&[]);
        #[cfg(windows)]
        if let Ok(mut job) = self.job.lock() {
            *job = None;
        }
    }

    pub fn open(
        &self,
        app: &AppHandle,
        state: &AppState,
        project_id: &str,
        session_id: Option<String>,
        title: String,
    ) -> Result<PtyOpened, String> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err("Agent Dock 正在关闭，无法打开 DeepSeek Web。".into());
        }
        {
            let mut sessions = lock_sessions(&self.sessions);
            reap(&mut sessions);
            if let Some(existing) = sessions.values_mut().find(|item| item.project_id == project_id) {
                if session_id.is_some() {
                    existing.session_id = session_id.clone();
                }
                if !title.trim().is_empty() {
                    existing.title = title.clone();
                }
                return Ok(PtyOpened {
                    pty_id: existing.id.clone(),
                    key: existing.key.clone(),
                    reused: true,
                    session_id: session_id.clone().or_else(|| existing.session_id.clone()),
                    title: existing.title.clone(),
                    opened_at: existing.opened_at,
                    kind: "web".into(),
                    url: Some(existing.url.clone()),
                });
            }
        }

        let live = self.live_child_pids();
        let planned = reap_strays_with(&live, self.lock_records(), |pid| self.forget_lock_pid(pid))?;
        if let Some(message) = dsh_web_guard::conflict_text(&planned.conflicts) {
            return Err(message);
        }

        let project = find_project(state, project_id)?;
        if !Path::new(&project.path).is_dir() {
            return Err(format!("项目文件夹读不了：{}", project.path));
        }
        let exe = tools::resolve_binary(ToolId::Dsh, &state.settings)?;
        let (node, _node_ver) = platform::resolve_node(tools::dsh::MIN_NODE)?;
        let entry = tools::dsh::js_entry(&exe).ok_or_else(|| {
            "无法定位 dsh 的 Node 入口。请安装：npm i -g @deepseek-ai/dsh".to_string()
        })?;
        let port = pick_port()?;
        let args = vec![
            "web".to_string(),
            "--no-open".into(),
            "--port".into(),
            port.to_string(),
        ];
        let payload = serde_json::to_string(&args).map_err(|_| "无法编码 DeepSeek 启动参数。".to_string())?;

        let mut cmd = Command::new(&node);
        cmd.arg("--input-type=module")
            .arg("-e")
            .arg(tools::dsh::BOOTSTRAP)
            .env("AD_DSH_BIN", &entry)
            .env("AD_DSH_ARGS", payload)
            .env(
                "PATH",
                platform::path_with_prepend(
                    node.parent().unwrap_or(Path::new(".")),
                    &platform::augmented_path(),
                ),
            )
            .env("TERM", "xterm-256color")
            .current_dir(&project.path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd.env(OWNER_ENV, std::process::id().to_string());
        if let Some(secret) = dsh_keys::active_secret(app) {
            cmd.env("DEEPSEEK_API_KEY", secret);
        }
        for (key, value) in proxy_env(project.proxy_enabled, &project.proxy_url)? {
            cmd.env(key, value);
        }
        platform::prepare_command(&mut cmd);
        platform::apply_no_window(&mut cmd);

        let mut child = cmd
            .spawn()
            .map_err(|err| format!("无法启动 dsh web：{err}"))?;
        #[cfg(windows)]
        if let Ok(job) = self.job.lock() {
            if let Some(job) = job.as_ref() {
                let _ = job.assign(child.id());
            }
        }
        self.remember_lock_pid(child.id());
        if self.shutting_down.load(Ordering::SeqCst) {
            let _ = kill_tree(&mut child);
            self.forget_lock_pid(child.id());
            return Err("Agent Dock 正在关闭，无法打开 DeepSeek Web。".into());
        }
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let log = Arc::new(Mutex::new(String::new()));
        spawn_pipe(stdout, log.clone());
        spawn_pipe(stderr, log.clone());

        let started = Instant::now();
        let url = loop {
            if let Some(url) = parse_web_url(&log.lock().expect("dsh web log")) {
                break url;
            }
            if started.elapsed() > URL_WAIT {
                let pid = child.id();
                let _ = kill_tree(&mut child);
                self.forget_lock_pid(pid);
                let dump = log.lock().expect("dsh web log").clone();
                let tail = dump.chars().rev().take(1200).collect::<String>().chars().rev().collect::<String>();
                return Err(if tail.trim().is_empty() {
                    "dsh web 启动超时，没有给出页面地址。".into()
                } else {
                    format!("dsh web 启动失败：{}", explain_boot_log(&tail))
                });
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.forget_lock_pid(child.id());
                    let dump = log.lock().expect("dsh web log").clone();
                    return Err(format!(
                        "dsh web 已退出（{status}）。{}",
                        explain_boot_log(
                            &dump.chars().rev().take(800).collect::<String>().chars().rev().collect::<String>()
                        )
                    ));
                }
                Ok(None) => thread::sleep(Duration::from_millis(120)),
                Err(err) => return Err(format!("无法确认 dsh web 状态：{err}")),
            }
        };

        let id = Uuid::new_v4().to_string();
        let opened_at = now_millis();
        let display = if title.trim().is_empty() {
            "DeepSeek Web".into()
        } else {
            title
        };
        let key = format!("{project_id}|dsh|web");
        if self.shutting_down.load(Ordering::SeqCst) {
            let _ = kill_tree(&mut child);
            self.forget_lock_pid(child.id());
            return Err("Agent Dock 正在关闭，无法打开 DeepSeek Web。".into());
        }
        {
            let mut sessions = lock_sessions(&self.sessions);
            sessions.insert(
                id.clone(),
                WebSession {
                    id: id.clone(),
                    key: key.clone(),
                    project_id: project_id.to_string(),
                    session_id: session_id.clone(),
                    title: display.clone(),
                    url: url.clone(),
                    opened_at,
                    child,
                },
            );
        }
        Ok(PtyOpened {
            pty_id: id,
            key,
            reused: false,
            session_id,
            title: display,
            opened_at,
            kind: "web".into(),
            url: Some(url),
        })
    }

    pub fn bind_session(
        &self,
        id: &str,
        session_id: &str,
        title: Option<String>,
    ) -> Result<Option<LivePtyInfo>, String> {
        let session_id = session_id.trim();
        if session_id.is_empty() {
            return Err("会话还没有编号，稍后再试".into());
        }
        let mut sessions = lock_sessions(&self.sessions);
        reap(&mut sessions);
        let Some(session) = sessions.get_mut(id) else {
            return Ok(None);
        };
        session.session_id = Some(session_id.to_string());
        if let Some(title) = title {
            let title = title.trim();
            if !title.is_empty() {
                session.title = title.to_string();
            }
        }
        Ok(Some(live_info(session)))
    }

    pub fn kill(&self, id: &str) -> Result<bool, String> {
        let mut sessions = lock_sessions(&self.sessions);
        let Some(mut session) = sessions.remove(id) else {
            return Ok(false);
        };
        let pid = session.child.id();
        kill_tree(&mut session.child)
            .map_err(|err| format!("无法关闭 DeepSeek Web：{err}"))?;
        drop(sessions);
        self.forget_lock_pid(pid);
        Ok(true)
    }

    fn live_child_pids(&self) -> Vec<u32> {
        let mut sessions = lock_sessions(&self.sessions);
        reap(&mut sessions);
        sessions.values().map(|item| item.child.id()).collect()
    }

    fn lock_records(&self) -> Vec<PidRecord> {
        let Ok(path) = self.lock_path.lock() else {
            return Vec::new();
        };
        let Some(path) = path.as_ref() else {
            return Vec::new();
        };
        read_lock_records(path)
    }

    fn remember_lock_pid(&self, pid: u32) {
        let mut records = self.lock_records();
        records.retain(|item| item.pid != pid);
        records.push(PidRecord {
            pid,
            owner_pid: std::process::id(),
        });
        self.write_lock_records(&records);
    }

    fn forget_lock_pid(&self, pid: u32) {
        let mut records = self.lock_records();
        records.retain(|item| item.pid != pid);
        self.write_lock_records(&records);
    }

    fn write_lock_records(&self, records: &[PidRecord]) {
        let Ok(path) = self.lock_path.lock() else {
            return;
        };
        let Some(path) = path.as_ref() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string(records) {
            let _ = fs::write(path, text);
        }
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn shutdown_app(app: &AppHandle) {
    if let Some(hub) = app.try_state::<Arc<Hub>>() {
        hub.shutdown();
    }
    let _ = crate::dsh_embed::close(app);
}

fn lock_sessions(lock: &Mutex<HashMap<String, WebSession>>) -> std::sync::MutexGuard<'_, HashMap<String, WebSession>> {
    lock.lock().unwrap_or_else(|err| err.into_inner())
}

fn live_info(session: &WebSession) -> LivePtyInfo {
    LivePtyInfo {
        pty_id: session.id.clone(),
        key: session.key.clone(),
        project_id: session.project_id.clone(),
        tool_id: ToolId::Dsh,
        session_id: session.session_id.clone(),
        title: session.title.clone(),
        alive: true,
        opened_at: session.opened_at,
        kind: "web".into(),
        url: Some(session.url.clone()),
    }
}

fn reap_strays_with(
    live: &[u32],
    records: Vec<PidRecord>,
    mut forget: impl FnMut(u32),
) -> Result<dsh_web_guard::Plan, String> {
    let procs = list_dsh_web_procs();
    let running = running_set(std::process::id(), &procs);
    let mut planned = dsh_web_guard::plan(&procs, std::process::id(), live, &running);
    for pid in dsh_web_guard::stale_owned_pids(&records, &running) {
        if !planned.reap.contains(&pid) && !live.contains(&pid) {
            planned.reap.push(pid);
        }
    }
    for pid in &planned.reap {
        kill_pid(*pid);
        forget(*pid);
    }
    Ok(planned)
}

fn list_dsh_web_procs() -> Vec<dsh_web_guard::Proc> {
    dsh_web_guard::parse_proc_table(&list_process_table())
}

fn list_process_table() -> String {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OutputEncoding = [Text.UTF8Encoding]::UTF8; Get-CimInstance Win32_Process -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^(node|dsh)' } | ForEach-Object { '{0}`t{1}`t{2}' -f $_.ProcessId, $_.ParentProcessId, (([string]$_.CommandLine) -replace '[\\t\\r\\n]+',' ') }",
        ]);
        platform::apply_no_window(&mut cmd);
        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();
        output
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
            .unwrap_or_default()
    }
    #[cfg(not(windows))]
    {
        let output = Command::new("ps")
            .args(["-ax", "-o", "pid=", "-o", "ppid=", "-o", "command="])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();
        let text = output
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
            .unwrap_or_default();
        text.lines()
            .filter_map(|line| {
                let line = line.trim();
                let mut bits = line.split_whitespace();
                let pid = bits.next()?;
                let ppid = bits.next()?;
                let cmd = bits.collect::<Vec<_>>().join(" ");
                Some(format!("{pid}\t{ppid}\t{cmd}"))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn running_set(our_pid: u32, procs: &[dsh_web_guard::Proc]) -> Vec<u32> {
    let mut ids = vec![our_pid];
    for proc in procs {
        ids.push(proc.pid);
        if pid_is_alive(proc.parent_pid) {
            ids.push(proc.parent_pid);
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn pid_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
        use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let state = unsafe { WaitForSingleObject(handle, 0) };
        unsafe {
            CloseHandle(handle);
        }
        state == WAIT_TIMEOUT
    }
    #[cfg(not(windows))]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

fn read_lock_records(path: &Path) -> Vec<PidRecord> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn spawn_pipe<T: std::io::Read + Send + 'static>(pipe: Option<T>, log: Arc<Mutex<String>>) {
    let Some(pipe) = pipe else { return };
    thread::spawn(move || {
        let reader = BufReader::new(pipe);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if let Ok(mut buf) = log.lock() {
                buf.push_str(&line);
                buf.push('\n');
            }
        }
    });
}

fn reap(sessions: &mut HashMap<String, WebSession>) {
    let mut dead = Vec::new();
    for (id, session) in sessions.iter_mut() {
        if let Ok(Some(_)) = session.child.try_wait() {
            dead.push(id.clone());
        }
    }
    for id in dead {
        sessions.remove(&id);
    }
}

fn kill_tree(child: &mut Child) -> Result<(), String> {
    kill_pid(child.id());
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

fn kill_pid(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        platform::apply_no_window(&mut cmd);
        let _ = cmd.status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(windows)]
struct KillJob {
    handle: isize,
}

#[cfg(windows)]
unsafe impl Send for KillJob {}

#[cfg(windows)]
impl KillJob {
    fn new() -> Option<Self> {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return None;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok == 0 {
            unsafe {
                CloseHandle(handle);
            }
            return None;
        }
        Some(Self {
            handle: handle as isize,
        })
    }

    fn assign(&self, pid: u32) -> bool {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};
        let proc = unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid) };
        if proc.is_null() {
            return false;
        }
        let ok = unsafe { AssignProcessToJobObject(self.handle as _, proc) };
        unsafe {
            CloseHandle(proc);
        }
        ok != 0
    }
}

#[cfg(windows)]
impl Drop for KillJob {
    fn drop(&mut self) {
        if self.handle != 0 {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.handle as _);
            }
            self.handle = 0;
        }
    }
}

fn pick_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr().map(|addr| addr.port()))
        .map_err(|err| format!("无法分配 DeepSeek Web 端口：{err}"))
}

pub fn explain_boot_log(text: &str) -> String {
    if text.contains("createZstdCompress") {
        return "当前 Node 太旧，没有 zstd 压缩接口。DeepSeek Harness 需要 Node 22.15 或更高；官方 dsh 命令还需要 22.18。低于这个版本时，在终端里执行 dsh web 会立刻回到提示符，浏览器访问 127.0.0.1:3080 也会被拒绝。请升级 Node，或把更新的 node.exe 放到 PATH 前面。".into();
    }
    text.trim().to_string()
}

fn extract_http_url(text: &str) -> Option<String> {
    for needle in ["http://127.0.0.1:", "http://localhost:", "https://127.0.0.1:"] {
        if let Some(idx) = text.find(needle) {
            let rest = &text[idx..];
            let end = rest
                .find(|c: char| c.is_whitespace() || matches!(c, ')' | ']' | '"' | '\''))
                .unwrap_or(rest.len());
            let url = rest[..end].trim_end_matches(['.', ',', ';']);
            if url.len() > needle.len() {
                return Some(url.to_string());
            }
        }
    }
    None
}

pub fn parse_web_url(text: &str) -> Option<String> {
    let mut with_token = None;
    let mut fallback = None;
    for raw in text.lines() {
        if let Some((_, rest)) = raw.split_once("dsh web:") {
            if let Some(url) = extract_http_url(rest) {
                return Some(url);
            }
        }
        if let Some(url) = extract_http_url(raw) {
            if url.contains("token=") {
                with_token.get_or_insert(url);
            } else {
                fallback.get_or_insert(url);
            }
        }
    }
    with_token.or(fallback)
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_old_node_zstd_failure() {
        let log = "SyntaxError: The requested module 'node:zlib' does not provide an export named 'createZstdCompress'";
        let text = explain_boot_log(log);
        assert!(text.contains("22.15"));
        assert!(text.contains("dsh web"));
    }

    #[test]
    fn parses_official_startup_line() {
        let log = "loading plugins\ndsh web: http://127.0.0.1:3080/?token=abc.def\nready\n";
        assert_eq!(
            parse_web_url(log).as_deref(),
            Some("http://127.0.0.1:3080/?token=abc.def")
        );
    }

    #[test]
    fn parses_localhost_and_ignores_noise() {
        let log = "warn: skip\nopen http://localhost:4312/\n";
        assert_eq!(parse_web_url(log).as_deref(), Some("http://localhost:4312/"));
        assert_eq!(parse_web_url("no url here"), None);
    }

    #[test]
    fn windows_process_table_hides_powershell_console() {
        let src = include_str!("dsh_web.rs");
        let start = src
            .find("fn list_process_table")
            .expect("list_process_table should exist");
        let body = src[start..]
            .split("fn running_set")
            .next()
            .expect("running_set follows list_process_table");
        assert!(
            body.contains("apply_no_window"),
            "powershell from the packaged GUI exe must use CREATE_NO_WINDOW"
        );
    }

    #[test]
    fn prefers_dsh_web_line_over_proxy_url() {
        let log = "HTTP_PROXY=http://127.0.0.1:7890\ndsh web: http://127.0.0.1:3080/?token=abc.def\n";
        assert_eq!(
            parse_web_url(log).as_deref(),
            Some("http://127.0.0.1:3080/?token=abc.def")
        );
    }
}
