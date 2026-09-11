use crate::platform;
use crate::state;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorDevRequest {
    pub project_id: String,
    pub prompt: String,
    pub agent_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorDevResult {
    pub ok: bool,
    pub agent_id: String,
    pub status: String,
    pub text: String,
}

pub fn run(app: &AppHandle, req: CursorDevRequest) -> Result<CursorDevResult, String> {
    let state = state::load_state(app)?;
    let project = state::find_project(&state, &req.project_id)?;
    let api_key = state.settings.cursor_api_key.trim();
    if api_key.is_empty() {
        return Err("还没有 Cursor API Key。打开设置填写后才能自动开发；半自动审查不需要。".into());
    }
    let prompt = req.prompt.trim();
    if prompt.is_empty() {
        return Err("本轮任务不能为空".into());
    }
    let node = platform::which_cmd("node").ok_or_else(|| "找不到 Node.js，Cursor SDK 需要本机 node。".to_string())?;
    let script = resolve_cursor_script()?;
    let payload = json!({
        "apiKey": api_key,
        "cwd": project.path,
        "prompt": prompt,
        "agentId": req.agent_id.filter(|id| !id.trim().is_empty()),
    });
    let tmp = std::env::temp_dir().join(format!("agent-dock-cursor-{}.json", std::process::id()));
    fs::write(&tmp, payload.to_string()).map_err(|err| format!("无法写入 Cursor 任务：{err}"))?;
    let mut cmd = Command::new(node);
    cmd.arg(&script).arg(&tmp);
    if let Some(root) = agent_dock_root() {
        cmd.current_dir(&root);
        cmd.env("NODE_PATH", root.join("node_modules"));
    }
    platform::prepare_command(&mut cmd);
    platform::apply_no_window(&mut cmd);
    let output = cmd
        .output()
        .map_err(|err| format!("无法启动 Cursor SDK：{err}"))?;
    let _ = fs::remove_file(&tmp);
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.trim().starts_with('{'))
        .ok_or_else(|| {
            if stderr.is_empty() {
                "Cursor SDK 没有返回结果".to_string()
            } else {
                stderr.clone()
            }
        })?;
    let parsed: CursorDevResult =
        serde_json::from_str(line).map_err(|err| format!("无法解析 Cursor 结果：{err}\n{line}"))?;
    Ok(parsed)
}

fn resolve_cursor_script() -> Result<PathBuf, String> {
    let mut candidates = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/cursor_agent.mjs")];
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("scripts/cursor_agent.mjs"));
        candidates.push(cwd.join("agent-dock/scripts/cursor_agent.mjs"));
    }
    for path in candidates {
        if path.is_file() {
            return Ok(path);
        }
    }
    Err("找不到 scripts/cursor_agent.mjs".into())
}

fn agent_dock_root() -> Option<PathBuf> {
    let from_crate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    if from_crate.join("package.json").is_file() {
        return from_crate.canonicalize().ok().or(Some(from_crate));
    }
    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("package.json").is_file() && cwd.join("scripts").is_dir() {
            return Some(cwd);
        }
        let nested = cwd.join("agent-dock");
        if nested.join("package.json").is_file() {
            return Some(nested);
        }
    }
    None
}
