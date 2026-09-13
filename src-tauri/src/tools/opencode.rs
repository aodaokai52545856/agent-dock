use super::{RenameKind, SessionRow, ToolId, matches_cwd, parse_time_value, resolve_binary};
use crate::platform;
use crate::state::AppSettings;
use serde::Deserialize;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize)]
struct OpenCodeSession {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    updated: Option<serde_json::Value>,
    #[serde(default)]
    created: Option<serde_json::Value>,
    #[serde(default)]
    directory: Option<String>,
}

pub fn list_sessions(cwd: &str, settings: &AppSettings) -> Result<Vec<SessionRow>, String> {
    let bin = resolve_binary(ToolId::Opencode, settings)?;
    let mut cmd = Command::new(&bin);
    cmd.args(["session", "list", "--format", "json", "-n", "80"])
        .current_dir(cwd);
    platform::prepare_command(&mut cmd);
    platform::apply_no_window(&mut cmd);
    let output = output_with_timeout(cmd, Duration::from_secs(5))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "OpenCode 列出 session 失败。请在该目录手动运行 opencode session list --format json 查看原因。{}",
            trim_detail(&stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = parse_session_list_json(&stdout)?;
    let mut rows: Vec<SessionRow> = parsed
        .into_iter()
        .filter(|item| {
            item.directory
                .as_deref()
                .map(|dir| matches_cwd(dir, cwd))
                .unwrap_or(true)
        })
        .map(|item| {
            let updated = item
                .updated
                .as_ref()
                .and_then(parse_time_value)
                .or_else(|| item.created.as_ref().and_then(parse_time_value))
                .unwrap_or(0);
            let title = if item.title.trim().is_empty() {
                item.id.clone()
            } else {
                item.title
            };
            SessionRow {
                tool_id: ToolId::Opencode,
                id: item.id,
                title,
                cwd: item.directory.unwrap_or_else(|| cwd.to_string()),
                updated_at: updated,
                rename_kind: RenameKind::Overlay,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(rows)
}

pub fn delete_args(session_id: &str) -> [&str; 3] {
    ["session", "delete", session_id]
}

pub fn delete_session(cwd: &str, session_id: &str, settings: &AppSettings) -> Result<(), String> {
    let bin = resolve_binary(ToolId::Opencode, settings)?;
    let mut cmd = Command::new(&bin);
    cmd.args(delete_args(session_id)).current_dir(cwd);
    platform::prepare_command(&mut cmd);
    platform::apply_no_window(&mut cmd);
    let output = cmd
        .output()
        .map_err(|err| format!("无法启动 OpenCode：{err}。请确认命令可执行。"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "OpenCode 删除 session 失败。请升级 OpenCode 后重试，或手动运行 opencode session delete。{}",
            trim_detail(&stderr)
        ));
    }
    Ok(())
}

fn output_with_timeout(mut cmd: Command, timeout: Duration) -> Result<std::process::Output, String> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("无法启动 OpenCode：{err}。请确认命令可执行。"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_end(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_end(&mut stderr);
                }
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("OpenCode 列出 session 超时。请检查 opencode 是否卡住。".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(30)),
            Err(err) => return Err(format!("等待 OpenCode 失败：{err}")),
        }
    }
}

fn parse_session_list_json(text: &str) -> Result<Vec<OpenCodeSession>, String> {
    let trimmed = text.trim();
    // OpenCode 1.18+ prints nothing and exits 0 when the project has no sessions.
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(trimmed).or_else(|_| {
        let start = trimmed.find('[').ok_or_else(|| json_error(trimmed))?;
        let end = trimmed.rfind(']').ok_or_else(|| json_error(trimmed))?;
        if end < start {
            return Err(json_error(trimmed));
        }
        serde_json::from_str(&trimmed[start..=end]).map_err(|_| json_error(trimmed))
    })
}

fn json_error(sample: &str) -> String {
    let preview = sample.chars().take(80).collect::<String>();
    format!("OpenCode 返回的 session 列表不是 JSON。请升级 OpenCode 后重试。 {preview}")
}

fn trim_detail(stderr: &str) -> String {
    let line = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if line.is_empty() {
        String::new()
    } else {
        format!(" {line}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_uses_official_session_delete() {
        assert_eq!(
            delete_args("ses_abc"),
            ["session", "delete", "ses_abc"]
        );
    }

    #[test]
    fn empty_stdout_means_no_sessions() {
        assert!(parse_session_list_json("").unwrap().is_empty());
        assert!(parse_session_list_json("  \n").unwrap().is_empty());
    }

    #[test]
    fn parses_pretty_json_array() {
        let json = r#"[
  {"id":"ses_1","title":"Fix layout","directory":"D:\\idea_jidian_projects\\agent-dock"}
]"#;
        let rows = parse_session_list_json(json).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "ses_1");
        assert_eq!(rows[0].title, "Fix layout");
    }
}
