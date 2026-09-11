use crate::platform::{self, posix_single_quote};
use crate::proxy::proxy_env;
use crate::state::{AppState, find_project};
use crate::tools::{self, ToolId};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOpened {
    pub pty_id: String,
    pub key: String,
    pub reused: bool,
    pub session_id: Option<String>,
    pub title: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyChunk {
    pub pty_id: String,
    pub data: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyExit {
    pub pty_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePtyInfo {
    pub pty_id: String,
    pub key: String,
    pub project_id: String,
    pub tool_id: ToolId,
    pub session_id: Option<String>,
    pub title: String,
    pub alive: bool,
}

struct LivePty {
    id: String,
    key: String,
    project_id: String,
    tool_id: ToolId,
    session_id: Option<String>,
    title: String,
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub struct PtyHub {
    sessions: Mutex<HashMap<String, LivePty>>,
}

impl PtyHub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
        })
    }

    pub fn start_monitor(self: &Arc<Self>, app: AppHandle) {
        let hub = Arc::clone(self);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_millis(400));
            let mut dead = Vec::new();
            if let Ok(mut sessions) = hub.sessions.lock() {
                for (id, pty) in sessions.iter_mut() {
                    if let Ok(Some(_)) = pty.child.try_wait() {
                        dead.push(id.clone());
                    }
                }
                for id in &dead {
                    sessions.remove(id);
                }
            }
            for id in dead {
                let _ = app.emit("pty-exit", PtyExit { pty_id: id });
            }
        });
    }

    pub fn list(&self) -> Vec<LivePtyInfo> {
        let mut sessions = self.sessions.lock().expect("pty lock");
        reap_dead(&mut sessions);
        sessions
            .values()
            .map(|pty| LivePtyInfo {
                pty_id: pty.id.clone(),
                key: pty.key.clone(),
                project_id: pty.project_id.clone(),
                tool_id: pty.tool_id,
                session_id: pty.session_id.clone(),
                title: pty.title.clone(),
                alive: true,
            })
            .collect()
    }

    pub fn write(&self, pty_id: &str, data: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().expect("pty lock");
        let pty = sessions
            .get_mut(pty_id)
            .ok_or_else(|| "这个终端已经关闭，请重新打开 session".to_string())?;
        pty.writer
            .write_all(data.as_bytes())
            .and_then(|_| pty.writer.flush())
            .map_err(|err| format!("写入终端失败：{err}"))
    }

    pub fn resize(&self, pty_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self.sessions.lock().expect("pty lock");
        let pty = sessions
            .get(pty_id)
            .ok_or_else(|| "这个终端已经关闭，无法调整大小".to_string())?;
        let (cols, rows) = clamp_pty_size(cols, rows);
        pty.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| format!("调整终端大小失败：{err}"))
    }

    pub fn kill(&self, pty_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().expect("pty lock");
        if let Some(mut pty) = sessions.remove(pty_id) {
            let _ = pty.child.kill();
        }
        Ok(())
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
    ) -> Result<PtyOpened, String> {
        let key = match session_id.as_deref() {
            Some(id) => format!("{}|{}|{id}", project_id, tool_id.as_str()),
            None => format!("{}|{}|new:{}", project_id, tool_id.as_str(), Uuid::new_v4()),
        };
        {
            let mut sessions = self.sessions.lock().expect("pty lock");
            reap_dead(&mut sessions);
            if let Some(existing) = sessions.values().find(|pty| pty.key == key) {
                return Ok(PtyOpened {
                    pty_id: existing.id.clone(),
                    key,
                    reused: true,
                    session_id: existing.session_id.clone(),
                    title: existing.title.clone(),
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
        let shell = platform::resolve_shell(&state.settings.powershell_path)?;
        let env_pairs = proxy_env(project.proxy_enabled, &project.proxy_url)?;
        let extra_args = match session_id.as_deref() {
            Some(id) => tools::resume_args(tool_id, id),
            None => Vec::new(),
        };
        let kind = shell_kind(&shell);
        let launch = match kind {
            ShellKind::PowerShell => wrap_utf8_launch(&build_launch_command(&exe, &extra_args)),
            ShellKind::Fish | ShellKind::Posix => build_posix_launch(&exe, &extra_args),
        };

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

        let mut cmd = CommandBuilder::new(&shell);
        apply_shell_args(&mut cmd, kind, &shell, &launch);
        cmd.cwd(&project.path);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("PATH", platform::augmented_path());
        #[cfg(unix)]
        if std::env::var_os("LANG").is_none() && std::env::var_os("LC_ALL").is_none() {
            cmd.env("LANG", "en_US.UTF-8");
        }
        for (env_key, value) in env_pairs {
            cmd.env(env_key, value);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|err| format!("无法启动终端：{err}"))?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| format!("无法读取终端输出：{err}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|err| format!("无法写入终端：{err}"))?;

        let pty_id = Uuid::new_v4().to_string();
        let resolved_session = session_id.clone();
        let display_title = if title.trim().is_empty() {
            resolved_session
                .clone()
                .unwrap_or_else(|| "新 Session".into())
        } else {
            title
        };

        {
            let mut sessions = self.sessions.lock().expect("pty lock");
            sessions.insert(
                pty_id.clone(),
                LivePty {
                    id: pty_id.clone(),
                    key: key.clone(),
                    project_id: project_id.to_string(),
                    tool_id,
                    session_id: resolved_session.clone(),
                    title: display_title.clone(),
                    master: pair.master,
                    writer,
                    child,
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
                        let _ = app.emit(
                            "pty-data",
                            PtyChunk {
                                pty_id: emit_id.clone(),
                                data,
                            },
                        );
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(PtyOpened {
            pty_id,
            key,
            reused: false,
            session_id: resolved_session,
            title: display_title,
        })
    }
}

fn reap_dead(sessions: &mut HashMap<String, LivePty>) {
    let mut dead = Vec::new();
    for (id, pty) in sessions.iter_mut() {
        if let Ok(Some(_)) = pty.child.try_wait() {
            dead.push(id.clone());
        }
    }
    for id in dead {
        sessions.remove(&id);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellKind {
    PowerShell,
    Fish,
    Posix,
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

pub fn wrap_utf8_launch(launch: &str) -> String {
    format!(
        "$env:TERM='xterm-256color'; $env:COLORTERM='truecolor'; [Console]::InputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); {launch}"
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
        assert!(wrapped.ends_with("& 'grok.exe'"));
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
    fn keeps_incomplete_utf8_until_next_chunk() {
        let mut pending = Vec::new();
        let first = decode_utf8_stream(&mut pending, &[0xE4, 0xB8]);
        assert_eq!(first, "");
        assert_eq!(pending, vec![0xE4, 0xB8]);
        let second = decode_utf8_stream(&mut pending, &[0xAD]);
        assert_eq!(second, "中");
        assert!(pending.is_empty());
    }
}
