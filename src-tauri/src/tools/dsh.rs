use super::{
    BinaryProbe, RenameKind, SessionRow, ToolId, file_mtime_millis, resolve_binary,
};
use crate::platform;
use crate::state::AppSettings;
use serde::Serialize;
#[cfg(test)]
use super::{first_string, matches_cwd, truncate_title};
#[cfg(test)]
use std::fs;
#[cfg(test)]
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Session logs need Node's built-in zstd (`createZstdCompress`), added in 22.15.
/// Official `dsh` also needs 22.18 for `import.meta.main`; we call `runCli()` ourselves.
pub const MIN_NODE: crate::platform::NodeVersion = crate::platform::NodeVersion {
    major: 22,
    minor: 15,
    patch: 0,
};

/// Boots `runCli()` so dsh works on Node < 22.18, where `import.meta.main` is missing
/// and the published `bin.js` otherwise exits without doing anything.
pub const BOOTSTRAP: &str = r#"
import { pathToFileURL } from 'node:url'
const bin = process.env.AD_DSH_BIN
if (!bin) {
  console.error('没有找到 DeepSeek Harness。请先安装：npm i -g @deepseek-ai/dsh')
  process.exit(1)
}
const args = JSON.parse(process.env.AD_DSH_ARGS || '[]')
process.argv = [process.execPath, bin, ...args]
const { runCli } = await import(pathToFileURL(bin).href)
await runCli()
"#;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct InstallReport {
    pub path: Option<String>,
    pub checks: Vec<InstallCheck>,
}

impl InstallReport {
    pub fn ok(&self) -> bool {
        !self.checks.is_empty() && self.checks.iter().all(|item| item.ok)
    }

    pub fn log(&self) -> String {
        let mut lines = Vec::new();
        for item in &self.checks {
            let mark = if item.ok { "通过" } else { "未通过" };
            lines.push(format!("检查 {} … {mark}：{}", item.name, item.detail));
        }
        if self.ok() {
            lines.push("安装成功。Node、dsh、dsh web 三项检查都已通过。".into());
        } else {
            lines.push("未通过检查，不能算安装成功。".into());
        }
        lines.join("\n")
    }

    pub fn summary(&self) -> String {
        let failed: Vec<&str> = self
            .checks
            .iter()
            .filter(|item| !item.ok)
            .map(|item| item.name.as_str())
            .collect();
        if failed.is_empty() {
            "Node、dsh、dsh web 检查通过。".into()
        } else {
            format!(
                "DeepSeek 未就绪（{}）。{}",
                failed.join("、"),
                self.checks
                    .iter()
                    .map(|item| format!("{}：{}", item.name, item.detail))
                    .collect::<Vec<_>>()
                    .join("；")
            )
        }
    }
}

pub fn looks_like_dsh_help(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("usage: dsh") && t.contains("commands:") && t.contains("web")
}

pub fn looks_like_dsh_web_help(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    t.contains("--no-open") && (t.contains("--port") || t.contains("web ui") || t.contains("browser ui"))
}

pub fn verify_install(settings: &AppSettings) -> InstallReport {
    let mut checks = Vec::new();
    let node = match platform::resolve_node(MIN_NODE) {
        Ok((exe, version)) => {
            checks.push(InstallCheck {
                name: "Node".into(),
                ok: true,
                detail: format!("{version}（{}）", exe.display()),
            });
            Some(exe)
        }
        Err(err) => {
            checks.push(InstallCheck {
                name: "Node".into(),
                ok: false,
                detail: err,
            });
            None
        }
    };

    let exe = resolve_binary(ToolId::Dsh, settings).ok();
    let path = exe.as_ref().map(|item| item.display().to_string());
    let entry = exe.as_ref().and_then(|item| js_entry(item));

    match (node.as_ref(), entry.as_ref()) {
        (Some(node), Some(entry)) => {
            match run_cli(node, entry, &["--help"]) {
                Ok(text) if looks_like_dsh_help(&text) => checks.push(InstallCheck {
                    name: "dsh".into(),
                    ok: true,
                    detail: "命令可用（dsh --help）".into(),
                }),
                Ok(text) if text.trim().is_empty() => checks.push(InstallCheck {
                    name: "dsh".into(),
                    ok: false,
                    detail: "命令没有输出。Node 过旧时官方入口会直接退出。".into(),
                }),
                Ok(text) => checks.push(InstallCheck {
                    name: "dsh".into(),
                    ok: false,
                    detail: clip_detail(&text),
                }),
                Err(err) => checks.push(InstallCheck {
                    name: "dsh".into(),
                    ok: false,
                    detail: err,
                }),
            }
            match run_cli(node, entry, &["web", "--help"]) {
                Ok(text) if looks_like_dsh_web_help(&text) => checks.push(InstallCheck {
                    name: "dsh web".into(),
                    ok: true,
                    detail: "命令可用（dsh web --help）".into(),
                }),
                Ok(text) if text.trim().is_empty() => checks.push(InstallCheck {
                    name: "dsh web".into(),
                    ok: false,
                    detail: "命令没有输出。Node 过旧时官方入口会直接退出，3080 也打不开。".into(),
                }),
                Ok(text) => checks.push(InstallCheck {
                    name: "dsh web".into(),
                    ok: false,
                    detail: clip_detail(&text),
                }),
                Err(err) => checks.push(InstallCheck {
                    name: "dsh web".into(),
                    ok: false,
                    detail: err,
                }),
            }
        }
        _ => {
            if entry.is_none() {
                let detail = if path.is_some() {
                    "找到了 dsh，但无法定位 Node 入口。请安装：npm i -g @deepseek-ai/dsh".to_string()
                } else {
                    "没有找到 dsh 命令。请安装：npm i -g @deepseek-ai/dsh".to_string()
                };
                checks.push(InstallCheck {
                    name: "dsh".into(),
                    ok: false,
                    detail: detail.clone(),
                });
                checks.push(InstallCheck {
                    name: "dsh web".into(),
                    ok: false,
                    detail,
                });
            } else {
                let detail = "没有可用的 Node，无法检测 dsh / dsh web。".to_string();
                checks.push(InstallCheck {
                    name: "dsh".into(),
                    ok: false,
                    detail: detail.clone(),
                });
                checks.push(InstallCheck {
                    name: "dsh web".into(),
                    ok: false,
                    detail,
                });
            }
        }
    }

    InstallReport { path, checks }
}

pub fn probe(settings: &AppSettings) -> BinaryProbe {
    let report = verify_install(settings);
    let path = report.path.clone();
    if report.ok() {
        BinaryProbe {
            found: true,
            path,
            message: None,
        }
    } else {
        BinaryProbe {
            found: false,
            path,
            message: Some(report.summary()),
        }
    }
}

fn clip_detail(text: &str) -> String {
    let trimmed = text.trim().replace('\n', " ");
    if trimmed.chars().count() <= 160 {
        return trimmed;
    }
    let mut out: String = trimmed.chars().take(159).collect();
    out.push('…');
    out
}

fn run_cli(node: &Path, entry: &Path, args: &[&str]) -> Result<String, String> {
    let payload =
        serde_json::to_string(&args).map_err(|_| "无法编码 DeepSeek 检测参数。".to_string())?;
    let mut cmd = Command::new(node);
    cmd.arg("--input-type=module")
        .arg("-e")
        .arg(BOOTSTRAP)
        .env("AD_DSH_BIN", entry)
        .env("AD_DSH_ARGS", payload)
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = node.parent() {
        cmd.env(
            "PATH",
            platform::path_with_prepend(dir, &platform::augmented_path()),
        );
    }
    platform::prepare_command(&mut cmd);
    platform::apply_no_window(&mut cmd);
    let output = cmd
        .output()
        .map_err(|err| format!("无法启动 dsh：{err}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !text.ends_with('\n') && !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&stderr);
    }
    if text.trim().is_empty() {
        return Err("命令没有输出。Node 过旧时官方入口会直接退出。".into());
    }
    Ok(text)
}

pub fn js_entry(exe: &Path) -> Option<PathBuf> {
    if exe
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("bin.js"))
        && exe.is_file()
    {
        return Some(exe.to_path_buf());
    }
    let dir = exe.parent()?;
    let npm = dir
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join("lib")
        .join("bin.js");
    npm.is_file().then_some(npm)
}

pub const FIXED_SESSION_ID: &str = "deepseek";
pub const FIXED_SESSION_TITLE: &str = "deepseek";

#[cfg(test)]
pub fn encode_cwd(cwd: &str) -> String {
    let mut text = cwd.trim().trim_matches('"').replace('\\', "/");
    while text.len() > 1 && text.ends_with('/') {
        text.pop();
    }
    let trimmed = text.trim_start_matches('/');
    format!("--{}--", trimmed.replace(['/', ':'], "-"))
}

pub fn list_sessions(cwd: &str) -> Result<Vec<SessionRow>, String> {
    Ok(vec![SessionRow {
        tool_id: ToolId::Dsh,
        id: FIXED_SESSION_ID.into(),
        title: FIXED_SESSION_TITLE.into(),
        cwd: cwd.to_string(),
        updated_at: file_mtime_millis(Path::new(cwd)),
        rename_kind: RenameKind::Overlay,
    }])
}

#[cfg(test)]
pub fn list_sessions_in(root: &Path, cwd: &str) -> Result<Vec<SessionRow>, String> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::new();
    let encoded = encode_cwd(cwd);
    let mut dirs = Vec::new();
    let preferred = root.join(&encoded);
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
    for project_dir in dirs {
        let in_preferred = project_dir.file_name().and_then(|name| name.to_str()) == Some(encoded.as_str());
        let Ok(sessions) = fs::read_dir(&project_dir) else {
            continue;
        };
        for session in sessions.flatten() {
            let dir = session.path();
            if !dir.is_dir() {
                continue;
            }
            let id = session.file_name().to_string_lossy().to_string();
            if id.is_empty() {
                continue;
            }
            let log = session_log_file(&dir);
            let Some(log) = log else {
                continue;
            };
            let (title, session_cwd, updated_at) = read_session_meta(&log, &id, cwd);
            if !in_preferred && !matches_cwd(&session_cwd, cwd) {
                continue;
            }
            rows.push(SessionRow {
                tool_id: ToolId::Dsh,
                id,
                title,
                cwd: cwd.to_string(),
                updated_at,
                rename_kind: RenameKind::Overlay,
            });
        }
    }
    Ok(rows)
}

pub fn delete_session(_cwd: &str, _session_id: &str) -> Result<(), String> {
    Err("DeepSeek 入口不能删除。".into())
}

#[cfg(test)]
fn session_log_file(dir: &Path) -> Option<PathBuf> {
    for name in [
        "session.jsonl",
        "session.v3.jsonl",
        "session.v2.jsonl",
        "session.jsonl.zstd",
        "session.v3.jsonl.zstd",
        "session.v2.jsonl.zstd",
    ] {
        let path = dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
fn read_session_meta(path: &Path, id: &str, fallback_cwd: &str) -> (String, String, i64) {
    let updated_at = file_mtime_millis(path);
    let Some(name) = path.file_name().and_then(|item| item.to_str()) else {
        return (truncate_title(id, 48), fallback_cwd.to_string(), updated_at);
    };
    if name.ends_with(".zstd") {
        return (truncate_title(id, 48), fallback_cwd.to_string(), updated_at);
    }
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut title = None;
    let mut session_cwd = fallback_cwd.to_string();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match value.get("type").and_then(|item| item.as_str()) {
            Some("session") => {
                if let Some(found) = value.get("cwd").and_then(|item| item.as_str()) {
                    if !found.trim().is_empty() {
                        session_cwd = found.trim().to_string();
                    }
                }
                if let Some(found) = first_string(&value, &["title", "name"]) {
                    title = Some(found);
                }
            }
            Some("message") | Some("user") if title.is_none() => {
                let message = value.get("message").unwrap_or(&value);
                if message.get("role").and_then(|item| item.as_str()) == Some("user")
                    || value.get("type").and_then(|item| item.as_str()) == Some("user")
                {
                    title = user_text(message).or_else(|| user_text(&value));
                }
            }
            _ => {}
        }
        if title.is_some() && session_cwd != fallback_cwd {
            break;
        }
    }
    (
        title
            .map(|text| truncate_title(&text, 48))
            .unwrap_or_else(|| truncate_title(id, 48)),
        session_cwd,
        updated_at,
    )
}

#[cfg(test)]
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
    fn bootstrap_calls_run_cli_and_does_not_install_tui() {
        assert!(BOOTSTRAP.contains("runCli"));
        assert!(!BOOTSTRAP.contains("plugin"));
        assert!(!BOOTSTRAP.contains("tui"));
        assert_eq!(MIN_NODE.major, 22);
        assert_eq!(MIN_NODE.minor, 15);
    }

    #[test]
    fn dsh_help_and_web_help_are_distinct() {
        let launcher = "Usage: dsh [options] [command] [args...]\n\nCommands:\n  web [options] [args...]        boot the web profile\n";
        let web = "Usage: dsh --profile web [options]\n\nServe the DeepSeek Harness browser UI.\n\nOptions:\n  --no-open                      do not open the Web UI\n  --port <port>                  listen port\n";
        assert!(looks_like_dsh_help(launcher));
        assert!(!looks_like_dsh_web_help(launcher));
        assert!(looks_like_dsh_web_help(web));
        assert!(!looks_like_dsh_help(web));
        assert!(!looks_like_dsh_help(""));
        assert!(!looks_like_dsh_web_help(""));
    }

    #[test]
    fn install_ok_requires_node_dsh_and_web() {
        let missing_web = InstallReport {
            path: Some("dsh.cmd".into()),
            checks: vec![
                InstallCheck {
                    name: "Node".into(),
                    ok: true,
                    detail: "22.23.1".into(),
                },
                InstallCheck {
                    name: "dsh".into(),
                    ok: true,
                    detail: "ok".into(),
                },
                InstallCheck {
                    name: "dsh web".into(),
                    ok: false,
                    detail: "no output".into(),
                },
            ],
        };
        assert!(!missing_web.ok());
        assert!(missing_web.summary().contains("dsh web"));
        let all = InstallReport {
            path: Some("dsh.cmd".into()),
            checks: vec![
                InstallCheck {
                    name: "Node".into(),
                    ok: true,
                    detail: "22.23.1".into(),
                },
                InstallCheck {
                    name: "dsh".into(),
                    ok: true,
                    detail: "ok".into(),
                },
                InstallCheck {
                    name: "dsh web".into(),
                    ok: true,
                    detail: "ok".into(),
                },
            ],
        };
        assert!(all.ok());
        assert!(all.log().contains("安装成功"));
    }

    #[test]
    fn js_entry_follows_npm_global_shim() {
        let root = tempfile::tempdir().unwrap();
        let npm = root.path();
        let bin = npm
            .join("node_modules")
            .join("@deepseek-ai")
            .join("dsh")
            .join("lib")
            .join("bin.js");
        fs::create_dir_all(bin.parent().unwrap()).unwrap();
        fs::write(&bin, "export {}").unwrap();
        fs::write(npm.join("dsh.cmd"), "@echo off").unwrap();
        fs::write(npm.join("dsh"), "#!/usr/bin/env node").unwrap();
        assert_eq!(js_entry(&npm.join("dsh.cmd")).as_deref(), Some(bin.as_path()));
        assert_eq!(js_entry(&npm.join("dsh")).as_deref(), Some(bin.as_path()));
        assert_eq!(js_entry(&bin).as_deref(), Some(bin.as_path()));
        assert_eq!(js_entry(Path::new("/missing/dsh.cmd")), None);
    }

    #[test]
    fn encodes_cwd_like_dsh_project_dirs() {
        assert_eq!(encode_cwd("/Users/me/app"), "--Users-me-app--");
        assert_eq!(
            encode_cwd(r"D:\idea_jidian_projects\agent-dock"),
            "--D--idea_jidian_projects-agent-dock--"
        );
    }

    #[test]
    fn lists_one_fixed_session_per_project() {
        let rows = list_sessions(r"D:\idea_jidian_projects\agent-dock").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "deepseek");
        assert_eq!(rows[0].title, "deepseek");
        assert_eq!(rows[0].tool_id, ToolId::Dsh);
        assert_eq!(rows[0].rename_kind, RenameKind::Overlay);
    }

    #[test]
    fn refuses_to_delete_the_fixed_session() {
        let err = delete_session(r"D:\tmp", "deepseek").unwrap_err();
        assert!(err.contains("不能删除"));
    }

    #[test]
    fn lists_plain_jsonl_session_dir() {
        let root = std::env::temp_dir().join(format!("ad-dsh-{}", std::process::id()));
        let cwd = "/tmp/dsh-demo";
        let dir = root.join(encode_cwd(cwd)).join("session_dsh_1");
        fs::create_dir_all(&dir).unwrap();
        let mut file = fs::File::create(dir.join("session.jsonl")).unwrap();
        writeln!(
            file,
            r#"{{"type":"session","id":"session_dsh_1","cwd":"/tmp/dsh-demo","title":"Review tests"}}"#
        )
        .unwrap();
        let rows = list_sessions_in(&root, cwd).unwrap();
        fs::remove_dir_all(&root).ok();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "session_dsh_1");
        assert_eq!(rows[0].tool_id, ToolId::Dsh);
        assert!(rows[0].title.contains("Review tests"));
    }
}
