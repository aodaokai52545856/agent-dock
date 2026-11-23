use crate::platform::{self, posix_single_quote};
use crate::proxy::proxy_env;
use crate::state::{AppState, find_project};
use crate::tools::{self, ToolId};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOpened {
    pub account: Option<tools::SessionAccount>,
    pub pty_id: String,
    pub key: String,
    pub reused: bool,
    pub session_id: Option<String>,
    pub title: String,
    pub opened_at: i64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyChunk {
    pub pty_id: String,
    pub data: String,
    pub bytes: usize,
}

// Bound output in transit to the webview, including xterm's parse queue.
// The reader waits here rather than dropping terminal control sequences.
const OUTPUT_WINDOW: usize = 256 * 1024;
#[derive(Default)]
struct OutputFlow {
    state: Mutex<(usize, bool)>,
    ready: Condvar,
}
impl OutputFlow {
    fn reset(&self) {
        self.state.lock().unwrap().0 = 0;
        self.ready.notify_all();
    }
    fn reserve(&self, bytes: usize) -> bool {
        let mut state = self.state.lock().unwrap();
        while !state.1 && state.0 + bytes > OUTPUT_WINDOW {
            state = self.ready.wait(state).unwrap();
        }
        if state.1 { return false; }
        state.0 += bytes;
        true
    }
    fn acknowledge(&self, bytes: usize) {
        let mut state = self.state.lock().unwrap();
        state.0 = state.0.saturating_sub(bytes);
        self.ready.notify_all();
    }
    fn stop(&self) {
        self.state.lock().unwrap().1 = true;
        self.ready.notify_all();
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyExit {
    pub pty_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePtyInfo {
    pub account: Option<tools::SessionAccount>,
    pub pty_id: String,
    pub key: String,
    pub project_id: String,
    pub tool_id: ToolId,
    pub session_id: Option<String>,
    pub title: String,
    pub alive: bool,
    pub opened_at: i64,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
}

struct LivePty {
    id: String,
    key: String,
    project_id: String,
    tool_id: ToolId,
    account: Option<tools::SessionAccount>,
    session_id: Option<String>,
    title: String,
    opened_at: i64,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    output: Arc<OutputFlow>,
}

impl Drop for LivePty {
    fn drop(&mut self) { self.output.stop(); }
}

pub struct PtyHub {
    sessions: Mutex<HashMap<String, LivePty>>,
    stopped: AtomicBool,
}

impl PtyHub {
    pub fn claude_account_running(&self, id: &str) -> bool {
        self.reap_dead();
        let sessions = self.sessions.lock().expect("pty lock");
        sessions.values().any(|pty| pty.account.as_ref().is_some_and(|a| a.id == id))
    }

    pub fn stop_claude_account(&self, app: &AppHandle, account_id: &str) -> Result<(), String> {
        let ids: Vec<_> = self.sessions.lock().map_err(|_| "终端状态不可用，请重启应用。")?
            .values().filter(|p| p.account.as_ref().is_some_and(|a| a.id == account_id))
            .map(|p| p.id.clone()).collect();
        for id in ids {
            let removed = self.sessions.lock().map_err(|_| "终端状态不可用")?.remove(&id);
            let Some(mut pty) = removed else { continue };
            let result = (|| -> Result<(), String> {
                if pty.child.try_wait().map_err(|_| "无法检查此账号终端，请先手动关闭。")?.is_none() {
                    #[cfg(windows)]
                    {
                        let pid = pty.child.process_id().ok_or("无法定位此账号终端进程。")?;
                        let mut cmd = std::process::Command::new("taskkill.exe");
                        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
                        platform::apply_no_window(&mut cmd);
                        let output = cmd.output().map_err(|_| "无法关闭此账号终端进程树，请先手动关闭。")?;
                        if !output.status.success() && pty.child.try_wait().ok().flatten().is_none() {
                            return Err("此账号终端尚未退出，已取消操作。".into());
                        }
                    }
                    #[cfg(not(windows))]
                    pty.child.kill().map_err(|_| "无法关闭此账号终端，已取消操作。")?;
                    let deadline = std::time::Instant::now() + Duration::from_secs(5);
                    while pty.child.try_wait().map_err(|_| "无法确认此账号终端已退出。")?.is_none() {
                        if std::time::Instant::now() >= deadline { return Err("等待此账号终端退出超时，已取消操作。".into()); }
                        thread::sleep(Duration::from_millis(25));
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                self.sessions.lock().map_err(|_| "终端状态不可用")?.insert(id, pty);
                return Err(error);
            }
            drop(pty);
            let _ = app.emit("pty-exit", PtyExit { pty_id: id });
        }
        Ok(())
    }
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
            stopped: AtomicBool::new(false),
        })
    }

    pub fn start_monitor(self: &Arc<Self>, app: AppHandle) {
        let hub = Arc::downgrade(self);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(400));
            let Some(hub) = hub.upgrade() else { break };
            if hub.stopped.load(Ordering::Acquire) { break; }
            let mut dead = Vec::new();
            let mut retired = Vec::new();
            if let Ok(mut sessions) = hub.sessions.lock() {
                for (id, pty) in sessions.iter_mut() {
                    if let Ok(Some(_)) = pty.child.try_wait() {
                        dead.push(id.clone());
                    }
                }
                for id in &dead {
                    retired.extend(sessions.remove(id));
                }
            }
            drop(retired);
            for id in dead {
                let _ = app.emit("pty-exit", PtyExit { pty_id: id });
            }
        });
    }

    pub fn list(&self) -> Vec<LivePtyInfo> {
        self.reap_dead();
        let sessions = self.sessions.lock().expect("pty lock");
        sessions.values().map(live_info).collect()
    }

    pub fn write(&self, pty_id: &str, data: &str) -> Result<(), String> {
        let writer = {
            let sessions = self.sessions.lock().expect("pty lock");
            sessions.get(pty_id).ok_or("这个终端已经关闭，请重新打开 session")?.writer.clone()
        };
        let mut writer = writer.lock().map_err(|_| "终端输入不可用")?;
        writer
            .write_all(data.as_bytes())
            .and_then(|_| writer.flush())
            .map_err(|err| format!("写入终端失败：{err}"))
    }

    pub fn resize(&self, pty_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let master = {
            let sessions = self.sessions.lock().expect("pty lock");
            sessions.get(pty_id).ok_or("这个终端已经关闭，无法调整大小")?.master.clone()
        };
        let (cols, rows) = clamp_pty_size(cols, rows);
        let master = master.lock().map_err(|_| "终端尺寸不可用")?;
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| format!("调整终端大小失败：{err}"))
    }

    pub fn kill(&self, pty_id: &str) -> Result<(), String> {
        let removed = self.sessions.lock().expect("pty lock").remove(pty_id);
        if let Some(mut pty) = removed {
            pty.output.stop();
            #[cfg(windows)]
            if pty.child.try_wait().ok().flatten().is_none() {
                if let Some(pid)=pty.child.process_id() {
                    let mut command=std::process::Command::new("taskkill.exe");
                    command.args(["/PID",&pid.to_string(),"/T","/F"]);
                    platform::apply_no_window(&mut command);
                    let _=command.output();
                }
            }
            let _ = pty.child.kill();
            let _ = pty.child.wait();
        }
        Ok(())
    }

    pub fn acknowledge(&self, pty_id: &str, bytes: usize) {
        if let Some(pty) = self.sessions.lock().expect("pty lock").get(pty_id) {
            pty.output.acknowledge(bytes);
        }
    }

    pub fn reset_output(&self) {
        for pty in self.sessions.lock().expect("pty lock").values() {
            pty.output.reset();
        }
    }

    pub fn close_all(&self) {
        self.stopped.store(true, Ordering::Release);
        let ids: Vec<_> = self.sessions.lock().expect("pty lock").keys().cloned().collect();
        for id in ids { let _ = self.kill(&id); }
    }

    pub fn open(
        &self,
        app: AppHandle,
        state: &AppState,
        project_id: &str,
        tool_id: ToolId,
        session_id: Option<String>,
        title: String,
        cols: u16,
        rows: u16,
        ui_theme: Option<&str>,
        command_args: Option<Vec<String>>,
    ) -> Result<PtyOpened, String> {
        let key = match session_id.as_deref() {
            Some(id) => session_live_key(project_id, tool_id, id),
            None => format!("{}|{}|new:{}", project_id, tool_id.as_str(), Uuid::new_v4()),
        };
        {
            self.reap_dead();
            let sessions = self.sessions.lock().expect("pty lock");
            if let Some(existing) = sessions.values().find(|pty| pty.key == key) {
                return Ok(PtyOpened {
                    account: existing.account.clone(),
                    pty_id: existing.id.clone(),
                    key,
                    reused: true,
                    session_id: existing.session_id.clone(),
                    title: existing.title.clone(),
                    opened_at: existing.opened_at,
                    kind: "pty".into(),
                    url: None,
                });
            }
        }

        let project = find_project(state, project_id)?;
        if !Path::new(&project.path).is_dir() {
            return Err(format!(
                "项目文件夹读不了：{}。请确认路径还在，或重新添加项目。",
                project.path
            ));
        }
        let exe = tools::resolve_binary(tool_id, &state.settings)?;
        let env_pairs = proxy_env(project.proxy_enabled, &project.proxy_url)?;
        let claude = if tool_id == ToolId::Claude {
            let mut base = std::env::vars().collect::<std::collections::BTreeMap<_, _>>();
            base.extend(env_pairs.iter().cloned());
            Some(crate::claude_accounts::launch_context(&app, &project.path, session_id.as_deref(), &base)?)
        } else { None };
        let extra_args = {
            let mut args = command_args.unwrap_or_else(|| tools::launch_args_for(tool_id, session_id.as_deref(), Some(&exe)));
            args.extend(claude_theme_args(tool_id, ui_theme));
            args
        };
        let omp_profile = if tool_id == ToolId::Omp {
            extra_args.windows(2).find(|pair| pair[0] == "--profile").map(|pair| pair[1].clone())
        } else { None };

        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty({
                let (cols, rows) = clamp_pty_size(cols, rows);
                PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                }
            })
            .map_err(|err| format!("无法创建终端：{err}"))?;

        let mut cmd = if launches_directly(tool_id, &exe) {
            let mut cmd = CommandBuilder::new(&exe);
            cmd.args(&extra_args);
            cmd
        } else {
            let shell = platform::resolve_shell(&state.settings.powershell_path)?;
            let kind = shell_kind(&shell);
            let launch = match kind {
                ShellKind::PowerShell => {
                    let mut line = wrap_utf8_launch(&build_launch_command(&exe, &extra_args));
                    line = format!("$env:COLORFGBG='{}'; {line}", colorfgbg_for(tool_id, ui_theme));
                    if tool_id == ToolId::Grokbuild && state.settings.grok_follow_glass {
                        line = format!("{}{line}", grok_glass_ps_prefix());
                    }
                    line
                }
                ShellKind::Fish | ShellKind::Posix => build_posix_launch(&exe, &extra_args),
            };
            let launch = match &claude {
                Some(context) => claude_launch_env(&launch, kind, context),
                None => launch,
            };
            let mut cmd = CommandBuilder::new(&shell);
            apply_shell_args(&mut cmd, kind, &shell, &launch);
            cmd
        };
        cmd.cwd(&project.path);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("PATH", platform::augmented_path());
        scrub_host_terminal_env(&mut cmd);
        cmd.env("TERM_PROGRAM", "xterm.js");
        cmd.env("TERM_PROGRAM_VERSION", "5.5.0");
        cmd.env("COLORFGBG", colorfgbg_for(tool_id, ui_theme));
        apply_grok_glass_env(&mut cmd, tool_id, state.settings.grok_follow_glass);
        if tool_id == ToolId::Omp && project.proxy_enabled {
            cmd.env("PI_PROXY", &project.proxy_url);
        }
        if tool_id == ToolId::Dsh {
            if let Some(secret) = crate::dsh_keys::active_secret(&app) {
                cmd.env("DEEPSEEK_API_KEY", secret);
            }
        }
        #[cfg(unix)]
        if std::env::var_os("LANG").is_none() && std::env::var_os("LC_ALL").is_none() {
            cmd.env("LANG", "en_US.UTF-8");
        }
        for (env_key, value) in env_pairs {
            cmd.env(env_key, value);
        }
        if let Some(context) = &claude {
            for key in &context.remove { cmd.env_remove(key); }
            for (key, value) in &context.set { cmd.env(key, value); }
        }

        // Acquire fallible I/O handles before spawning, so failures cannot orphan a child.
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| format!("无法读取终端输出：{err}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|err| format!("无法写入终端：{err}"))?;
        let child = pair.slave.spawn_command(cmd)
            .map_err(|err| format!("无法启动终端：{err}"))?;
        let output = Arc::new(OutputFlow::default());

        let pty_id = Uuid::new_v4().to_string();
        let resolved_session = session_id.clone();
        let opened_at = now_millis();
        let display_title = if title.trim().is_empty() {
            resolved_session
                .clone()
                .unwrap_or_else(|| "新会话".into())
        } else {
            title
        };
        let account = claude.as_ref().map(|context| tools::SessionAccount {
            id: context.id.clone(),
            name: context.name.clone(),
        }).or_else(|| omp_profile.map(|profile| tools::SessionAccount { id:profile.clone(), name:profile }));

        {
            let mut sessions = self.sessions.lock().expect("pty lock");
            sessions.insert(
                pty_id.clone(),
                LivePty {
                    id: pty_id.clone(),
                    key: key.clone(),
                    project_id: project_id.to_string(),
                    tool_id,
                    account: account.clone(),
                    session_id: resolved_session.clone(),
                    title: display_title.clone(),
                    opened_at,
                    master: Arc::new(Mutex::new(pair.master)),
                    writer: Arc::new(Mutex::new(writer)),
                    child,
                    output: output.clone(),
                },
            );
        }

        let emit_id = pty_id.clone();
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut pending = Vec::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = decode_utf8_stream(&mut pending, &buf[..n]);
                        if data.is_empty() {
                            continue;
                        }
                        let bytes = data.len();
                        // On close, keep draining ConPTY until EOF so closing its
                        // master cannot deadlock waiting for its output pipe.
                        if !output.reserve(bytes) { continue; }
                        if app.emit(
                            "pty-data",
                            PtyChunk {
                                pty_id: emit_id.clone(),
                                data,
                                bytes,
                            },
                        ).is_err() { output.acknowledge(bytes); }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(PtyOpened {
            account,
            pty_id,
            key,
            reused: false,
            session_id: resolved_session,
            title: display_title,
            opened_at,
            kind: "pty".into(),
            url: None,
        })
    }

    pub fn bind_session(
        &self,
        pty_id: &str,
        session_id: &str,
        title: Option<String>,
    ) -> Result<LivePtyInfo, String> {
        let session_id = session_id.trim();
        if session_id.is_empty() {
            return Err("会话还没有编号，稍后再试".into());
        }
        self.reap_dead();
        let mut sessions = self.sessions.lock().expect("pty lock");
        let (project_id, tool_id) = {
            let pty = sessions
                .get(pty_id)
                .ok_or_else(|| "这个终端已经关闭，无法绑定会话".to_string())?;
            (pty.project_id.clone(), pty.tool_id)
        };
        let key = session_live_key(&project_id, tool_id, session_id);
        if let Some(existing) = sessions
            .values()
            .find(|pty| pty.key == key && pty.id != pty_id)
        {
            return Ok(live_info(existing));
        }
        let pty = sessions
            .get_mut(pty_id)
            .ok_or_else(|| "这个终端已经关闭，无法绑定会话".to_string())?;
        if let Some(current) = pty.session_id.as_deref() {
            if current != session_id {
                return Err("这个终端已经绑定了另一个会话".into());
            }
        }
        pty.session_id = Some(session_id.to_string());
        pty.key = key;
        if let Some(title) = title {
            let title = title.trim();
            if !title.is_empty() {
                pty.title = title.to_string();
            }
        }
        Ok(live_info(pty))
    }

    /// `bind_session` keeps a window on its session. This is for when the tool
    /// itself moved on, as `/clear` does.
    pub fn switch_session(&self, pty_id: &str, session_id: &str) -> Result<LivePtyInfo, String> {
        let session_id = session_id.trim();
        if session_id.is_empty() {
            return Err("会话还没有编号，稍后再试".into());
        }
        self.reap_dead();
        let mut sessions = self.sessions.lock().expect("pty lock");
        let (project_id, tool_id) = {
            let pty = sessions
                .get(pty_id)
                .ok_or_else(|| "这个终端已经关闭，无法绑定会话".to_string())?;
            (pty.project_id.clone(), pty.tool_id)
        };
        let key = session_live_key(&project_id, tool_id, session_id);
        if sessions.values().any(|pty| pty.key == key && pty.id != pty_id) {
            return Err("这个会话已经在另一个终端里打开".into());
        }
        let pty = sessions
            .get_mut(pty_id)
            .ok_or_else(|| "这个终端已经关闭，无法绑定会话".to_string())?;
        pty.session_id = Some(session_id.to_string());
        pty.key = key;
        Ok(live_info(pty))
    }

    fn reap_dead(&self) {
        let retired = {
            let mut sessions = self.sessions.lock().expect("pty lock");
            let dead: Vec<_> = sessions.iter_mut()
                .filter_map(|(id, pty)| matches!(pty.child.try_wait(), Ok(Some(_))).then(|| id.clone()))
                .collect();
            dead.into_iter().filter_map(|id| sessions.remove(&id)).collect::<Vec<_>>()
        };
        // Closing a ConPTY master can wait on its output drain.
        drop(retired);
    }
}

pub fn session_live_key(project_id: &str, tool_id: ToolId, session_id: &str) -> String {
    format!("{}|{}|{session_id}", project_id, tool_id.as_str())
}

fn live_info(pty: &LivePty) -> LivePtyInfo {
    LivePtyInfo {
        account: pty.account.clone(),
        pty_id: pty.id.clone(),
        key: pty.key.clone(),
        project_id: pty.project_id.clone(),
        tool_id: pty.tool_id,
        session_id: pty.session_id.clone(),
        title: pty.title.clone(),
        alive: true,
        opened_at: pty.opened_at,
        kind: "pty".into(),
        url: None,
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn launches_directly(tool_id: ToolId, exe: &Path) -> bool {
    // Native TUIs own the console directly, including its raw input modes.
    matches!(tool_id, ToolId::Omp | ToolId::Antigravity) && !matches!(
        exe.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).as_deref(),
        Some("cmd" | "bat" | "ps1")
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellKind {
    PowerShell,
    Fish,
    Posix,
}

// Reapply isolation after shell startup files. Otherwise a login shell's
// profile can silently replace the account or put a global API token back.
fn claude_launch_env(launch: &str, kind: ShellKind, ctx: &crate::claude_accounts::LaunchContext) -> String {
    match kind {
        ShellKind::PowerShell => {
            let mut prefix = String::new();
            for key in &ctx.remove { prefix.push_str(&format!("Remove-Item -LiteralPath 'Env:{key}' -ErrorAction SilentlyContinue; ")); }
            for (key, value) in &ctx.set { prefix.push_str(&format!("$env:{key}={}; ", ps_single_quote(value))); }
            format!("{prefix}{launch}")
        }
        ShellKind::Fish | ShellKind::Posix => {
            let mut prefix = vec!["env".to_string()];
            for key in &ctx.remove { prefix.extend(["-u".into(), posix_single_quote(key)]); }
            for (key, value) in &ctx.set { prefix.push(posix_single_quote(&format!("{key}={value}"))); }
            format!("{} {launch}", prefix.join(" "))
        }
    }
}

fn shell_kind(shell: &str) -> ShellKind {
    let name = Path::new(shell)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(shell)
        .to_ascii_lowercase();
    if name.contains("powershell") || name == "pwsh" || name == "pwsh.exe" {
        ShellKind::PowerShell
    } else if name.contains("fish") {
        ShellKind::Fish
    } else {
        ShellKind::Posix
    }
}

fn apply_shell_args(cmd: &mut CommandBuilder, kind: ShellKind, shell: &str, launch: &str) {
    match kind {
        ShellKind::PowerShell => {
            cmd.arg("-NoLogo");
            cmd.arg("-NoProfile");
            cmd.arg("-NoExit");
            cmd.arg("-Command");
            cmd.arg(launch);
        }
        ShellKind::Fish => {
            cmd.arg("-ilc");
            cmd.arg(format!("{}; exec {}", launch, posix_single_quote(shell)));
        }
        ShellKind::Posix => {
            cmd.arg("-il");
            cmd.arg("-c");
            cmd.arg(format!("{}; exec {}", launch, posix_single_quote(shell)));
        }
    }
}

pub fn ps_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn build_launch_command(exe: &Path, args: &[String]) -> String {
    let mut parts = vec![format!("& {}", ps_single_quote(&exe.to_string_lossy()))];
    for arg in args {
        parts.push(ps_single_quote(arg));
    }
    parts.join(" ")
}

#[cfg(test)]
fn cmd_needs_quotes(value: &str) -> bool {
    value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '"' | '&' | '|' | '<' | '>' | '^' | '%'))
}

#[cfg(test)]
fn quote_cmd_token(value: &str) -> String {
    if !cmd_needs_quotes(value) {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('"', "\\\""))
}

#[cfg(test)]
pub fn windows_cmd_line(exe: &Path, args: &[String]) -> String {
    let mut line = quote_cmd_token(&exe.to_string_lossy());
    for arg in args {
        line.push(' ');
        line.push_str(&quote_cmd_token(arg));
    }
    line
}

pub fn build_posix_launch(exe: &Path, args: &[String]) -> String {
    let mut parts = vec![posix_single_quote(&exe.to_string_lossy())];
    for arg in args {
        parts.push(posix_single_quote(arg));
    }
    parts.join(" ")
}

pub fn clamp_pty_size(cols: u16, rows: u16) -> (u16, u16) {
    (cols.max(20), rows.max(8))
}

pub const HOST_TERMINAL_ENV: &[&str] = &[
    "TERMINAL_EMULATOR",
    "TERM_PROGRAM",
    "TERM_PROGRAM_VERSION",
];

fn scrub_host_terminal_env(cmd: &mut CommandBuilder) {
    for key in HOST_TERMINAL_ENV {
        cmd.env_remove(key);
    }
}

pub fn resolved_pty_theme(ui_theme: Option<&str>) -> &'static str {
    match ui_theme.map(str::trim).unwrap_or("dark") {
        "light" => "light",
        _ => "dark",
    }
}

pub fn pty_forces_dark(tool_id: ToolId) -> bool {
    matches!(tool_id, ToolId::Opencode | ToolId::Kimi | ToolId::Pi)
}

pub fn colorfgbg(ui_theme: Option<&str>) -> &'static str {
    if resolved_pty_theme(ui_theme) == "light" {
        "0;15"
    } else {
        "15;0"
    }
}

pub fn colorfgbg_for(tool_id: ToolId, ui_theme: Option<&str>) -> &'static str {
    if pty_forces_dark(tool_id) {
        "15;0"
    } else {
        colorfgbg(ui_theme)
    }
}

pub fn claude_theme_settings_json(theme: &str) -> String {
    format!(r#"{{"theme":"{theme}"}}"#)
}

pub fn claude_theme_settings_path(theme: &str) -> std::path::PathBuf {
    std::env::temp_dir()
        .join("agent-dock")
        .join(format!("claude-theme-{theme}.json"))
}

fn write_claude_theme_settings(theme: &str) -> Option<std::path::PathBuf> {
    let path = claude_theme_settings_path(theme);
    let body = claude_theme_settings_json(theme);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(body.as_str()) {
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::write(&path, body).ok()?;
    }
    Some(path)
}

pub fn claude_theme_args(tool_id: ToolId, ui_theme: Option<&str>) -> Vec<String> {
    if tool_id != ToolId::Claude {
        return Vec::new();
    }
    let theme = resolved_pty_theme(ui_theme);
    // Inline JSON contains double quotes. PowerShell -Command is itself a Windows
    // argv, so those quotes get stripped and Claude sees `{theme:light}`.
    match write_claude_theme_settings(theme) {
        Some(path) => vec!["--settings".into(), path.to_string_lossy().into_owned()],
        None => Vec::new(),
    }
}

pub fn grok_glass_env(enabled: bool) -> &'static [(&'static str, &'static str)] {
    if enabled {
        &[
            ("GROK_TERMINAL_THEME", "1"),
            ("GROK_THEME", "terminal"),
            ("LC_GROK_THEME", "terminal"),
        ]
    } else {
        &[]
    }
}

fn apply_grok_glass_env(cmd: &mut CommandBuilder, tool_id: ToolId, enabled: bool) {
    if tool_id != ToolId::Grokbuild {
        return;
    }
    for (key, value) in grok_glass_env(enabled) {
        cmd.env(*key, *value);
    }
}

fn grok_glass_ps_prefix() -> String {
    grok_glass_env(true)
        .iter()
        .map(|(key, value)| format!("$env:{key}='{value}'; "))
        .collect()
}

pub fn wrap_utf8_launch(launch: &str) -> String {
    format!(
        "Remove-Item Env:TERMINAL_EMULATOR,Env:TERM_PROGRAM,Env:TERM_PROGRAM_VERSION -ErrorAction SilentlyContinue; $env:TERM='xterm-256color'; $env:COLORTERM='truecolor'; $env:TERM_PROGRAM='xterm.js'; $env:TERM_PROGRAM_VERSION='5.5.0'; [Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); {launch}"
    )
}

pub fn decode_utf8_stream(pending: &mut Vec<u8>, incoming: &[u8]) -> String {
    pending.extend_from_slice(incoming);
    let mut out = String::new();
    loop {
        match std::str::from_utf8(pending) {
            Ok(text) => {
                out.push_str(text);
                pending.clear();
                break;
            }
            Err(err) => {
                let valid = err.valid_up_to();
                if valid > 0 {
                    out.push_str(std::str::from_utf8(&pending[..valid]).expect("valid UTF-8 prefix"));
                    pending.drain(..valid);
                    continue;
                }
                match err.error_len() {
                    Some(len) => {
                        out.push(char::REPLACEMENT_CHARACTER);
                        pending.drain(..len);
                    }
                    None => break,
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_tuis_own_the_console_while_shell_shims_keep_shell_dispatch() {
        assert!(launches_directly(ToolId::Omp, Path::new("C:/bin/omp.exe")));
        assert!(launches_directly(ToolId::Omp, Path::new("/usr/local/bin/omp")));
        assert!(launches_directly(ToolId::Antigravity, Path::new("C:/bin/agy.exe")));
        assert!(launches_directly(ToolId::Antigravity, Path::new("/usr/local/bin/agy")));
        for name in ["tool.cmd", "tool.CMD", "tool.bat", "tool.ps1"] {
            assert!(!launches_directly(ToolId::Omp, Path::new(name)));
            assert!(!launches_directly(ToolId::Antigravity, Path::new(name)));
        }
        assert!(!launches_directly(ToolId::Pi, Path::new("pi.exe")));
    }

    #[test]
    fn claude_shell_launch_applies_isolation_after_profile_with_quoted_values() {
        let context = crate::claude_accounts::LaunchContext {
            id: "test".into(), name: "工作".into(),
            remove: vec!["ANTHROPIC_API_KEY".into(), "CLAUDE_CONFIG_DIR".into()],
            set: std::collections::BTreeMap::from([("CLAUDE_CONFIG_DIR".into(), "C:/profiles/O'Brien work".into())]),
        };
        let ps = claude_launch_env("& 'claude.exe'", ShellKind::PowerShell, &context);
        assert!(ps.contains("'C:/profiles/O''Brien work'"));
        assert!(ps.ends_with("& 'claude.exe'"));
        let sh = claude_launch_env("'claude'", ShellKind::Posix, &context);
        assert!(sh.starts_with("env -u 'ANTHROPIC_API_KEY'"));
        assert!(sh.ends_with(" 'claude'"));
    }

    #[cfg(windows)]
    #[test]
    fn native_powershell_process_cannot_inherit_another_claude_identity() {
        let context = crate::claude_accounts::LaunchContext {
            id: "test".into(), name: "工作".into(),
            remove: vec!["ANTHROPIC_API_KEY".into(), "CLAUDE_CONFIG_DIR".into(), "HTTPS_PROXY".into()],
            set: std::collections::BTreeMap::from([("CLAUDE_CONFIG_DIR".into(), "C:/profiles/O'Brien work".into())]),
        };
        let probe = "@{ config=$env:CLAUDE_CONFIG_DIR; key=$env:ANTHROPIC_API_KEY; proxy=$env:HTTPS_PROXY } | ConvertTo-Json -Compress";
        let launch = claude_launch_env(probe, ShellKind::PowerShell, &context);
        let output = std::process::Command::new("powershell.exe").args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &launch])
            .env("ANTHROPIC_API_KEY", "fake-global-secret").env("CLAUDE_CONFIG_DIR", "wrong-account").env("HTTPS_PROXY", "http://wrong-proxy:1234")
            .output().unwrap();
        assert!(output.status.success());
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["config"], "C:/profiles/O'Brien work");
        assert!(result["key"].is_null()); assert!(result["proxy"].is_null());
    }

    #[test]
    fn windows_cmd_line_quotes_spaces() {
        let line = windows_cmd_line(
            Path::new(r"C:\Program Files\npm\dsh.cmd"),
            &["web".into(), "--no-open".into()],
        );
        assert_eq!(line, r#""C:\Program Files\npm\dsh.cmd" web --no-open"#);
    }

    #[test]
    fn windows_cmd_line_leaves_plain_npm_shim_unquoted() {
        let line = windows_cmd_line(
            Path::new(r"C:\Users\me\AppData\Roaming\npm\dsh.cmd"),
            &["web".into(), "--no-open".into()],
        );
        assert_eq!(
            line,
            r"C:\Users\me\AppData\Roaming\npm\dsh.cmd web --no-open"
        );
    }

    #[test]
    fn claude_launch_follows_dock_theme_for_one_session() {
        assert!(claude_theme_args(ToolId::Grokbuild, Some("light")).is_empty());
        let light = claude_theme_args(ToolId::Claude, Some("light"));
        assert_eq!(light[0], "--settings");
        assert!(
            light[1].ends_with("claude-theme-light.json"),
            "expected a settings file path, got {}",
            light[1]
        );
        assert_eq!(
            std::fs::read_to_string(&light[1]).expect("theme file"),
            r#"{"theme":"light"}"#
        );
        let dark = claude_theme_args(ToolId::Claude, Some("dark"));
        assert!(dark[1].ends_with("claude-theme-dark.json"));
        assert_eq!(colorfgbg(Some("light")), "0;15");
        assert_eq!(colorfgbg(Some("dark")), "15;0");
        assert_eq!(colorfgbg_for(ToolId::Grokbuild, Some("light")), "0;15");
        assert_eq!(colorfgbg_for(ToolId::Opencode, Some("light")), "15;0");
        assert_eq!(colorfgbg_for(ToolId::Kimi, Some("light")), "15;0");
        let cmd = build_launch_command(
            Path::new(r"C:\Users\me\AppData\Roaming\npm\claude.cmd"),
            &light,
        );
        assert!(cmd.contains("'--settings'"));
        assert!(
            !cmd.contains(r#"{"theme""#),
            "inline JSON breaks powershell -Command quoting on Windows: {cmd}"
        );
        assert!(cmd.contains("claude-theme-light.json"));
    }

    #[test]
    fn grok_glass_env_only_when_enabled() {
        assert!(grok_glass_env(false).is_empty());
        let pairs = grok_glass_env(true);
        assert!(pairs.iter().any(|(k, v)| *k == "GROK_TERMINAL_THEME" && *v == "1"));
        assert!(pairs.iter().any(|(k, v)| *k == "GROK_THEME" && *v == "terminal"));
        assert!(pairs.iter().any(|(k, v)| *k == "LC_GROK_THEME" && *v == "terminal"));
        let prefix = grok_glass_ps_prefix();
        assert!(prefix.contains("$env:GROK_THEME='terminal'"));
        assert!(prefix.contains("$env:GROK_TERMINAL_THEME='1'"));
    }

    #[test]
    fn powershell_launch_can_call_npm_cmd_shim() {
        let cmd = build_launch_command(
            Path::new(r"C:\Users\PS\AppData\Roaming\npm\dsh.cmd"),
            &["--profile".into(), "tui".into()],
        );
        assert_eq!(
            cmd,
            r"& 'C:\Users\PS\AppData\Roaming\npm\dsh.cmd' '--profile' 'tui'"
        );
    }

    #[test]
    fn quotes_powershell_paths_and_args() {
        let cmd = build_launch_command(
            Path::new(r"C:\Program Files\kimi.exe"),
            &["--session".into(), "session_it's".into()],
        );
        assert_eq!(
            cmd,
            r"& 'C:\Program Files\kimi.exe' '--session' 'session_it''s'"
        );
    }

    #[test]
    fn clamps_tiny_pty_size() {
        assert_eq!(clamp_pty_size(2, 1), (20, 8));
        assert_eq!(clamp_pty_size(120, 32), (120, 32));
    }

    #[test]
    fn wraps_launch_with_utf8_console() {
        let wrapped = wrap_utf8_launch("& 'grok.exe'");
        assert!(wrapped.contains("[Console]::OutputEncoding"));
        assert!(wrapped.contains("xterm-256color"));
        assert!(wrapped.contains("Remove-Item Env:TERMINAL_EMULATOR"));
        assert!(wrapped.contains("$env:TERM_PROGRAM='xterm.js'"));
        assert!(wrapped.ends_with("& 'grok.exe'"));
    }

    #[test]
    fn host_terminal_env_includes_jetbrains_identity() {
        assert!(HOST_TERMINAL_ENV.contains(&"TERMINAL_EMULATOR"));
        assert!(HOST_TERMINAL_ENV.contains(&"TERM_PROGRAM"));
    }

    #[test]
    fn quotes_posix_paths_and_args() {
        let cmd = build_posix_launch(
            Path::new("/Applications/My App/kimi"),
            &["--session".into(), "session_it's".into()],
        );
        assert_eq!(
            cmd,
            "'/Applications/My App/kimi' '--session' 'session_it'\\''s'"
        );
    }

    #[test]
    fn classifies_shells() {
        assert_eq!(shell_kind("powershell.exe"), ShellKind::PowerShell);
        assert_eq!(shell_kind("/bin/zsh"), ShellKind::Posix);
        assert_eq!(shell_kind("/opt/homebrew/bin/fish"), ShellKind::Fish);
    }

    #[test]
    fn session_live_key_is_stable_for_resume() {
        assert_eq!(
            session_live_key("proj", ToolId::Grokbuild, "abc"),
            "proj|grokbuild|abc"
        );
    }

    #[test]
    fn keeps_incomplete_utf8_until_next_chunk() {
        let mut pending = Vec::new();
        let first = decode_utf8_stream(&mut pending, &[0xE4, 0xB8]);
        assert_eq!(first, "");
        assert_eq!(pending, vec![0xE4, 0xB8]);
        let second = decode_utf8_stream(&mut pending, &[0xAD]);
        assert_eq!(second, "中");
        assert!(pending.is_empty());
    }

    #[test]
    fn output_flow_bounds_bursts_until_the_parser_acknowledges() {
        use std::sync::mpsc;
        let flow = Arc::new(OutputFlow::default());
        assert!(flow.reserve(OUTPUT_WINDOW));
        let producer = flow.clone();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || { tx.send(producer.reserve(8192)).unwrap(); });
        assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
        flow.acknowledge(8192);
        assert!(rx.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
        assert_eq!(flow.state.lock().unwrap().0, OUTPUT_WINDOW);
        flow.acknowledge(usize::MAX);
        assert_eq!(flow.state.lock().unwrap().0, 0);
    }

    #[test]
    fn output_flow_close_releases_a_blocked_reader() {
        use std::sync::mpsc;
        let flow = Arc::new(OutputFlow::default());
        assert!(flow.reserve(OUTPUT_WINDOW));
        let producer = flow.clone();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || { tx.send(producer.reserve(1)).unwrap(); });
        assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
        flow.stop();
        assert!(!rx.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
    }

    #[test]
    fn output_flow_reload_releases_credits_from_the_previous_webview() {
        let flow = OutputFlow::default();
        assert!(flow.reserve(OUTPUT_WINDOW));
        flow.reset();
        assert!(flow.reserve(OUTPUT_WINDOW));
        flow.stop();
        flow.reset();
        assert!(!flow.reserve(1)); // A reload cannot resurrect a closed terminal.
    }

    #[test]
    fn output_flow_survives_repeated_large_bursts_without_retaining_credits() {
        let flow = OutputFlow::default();
        for _ in 0..1_000 {
            for _ in 0..32 { assert!(flow.reserve(8192)); }
            assert_eq!(flow.state.lock().unwrap().0, OUTPUT_WINDOW);
            for _ in 0..32 { flow.acknowledge(8192); }
            assert_eq!(flow.state.lock().unwrap().0, 0);
        }
    }
}
