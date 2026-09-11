mod bridge;
mod cursor_dev;
mod grok_accounts;
mod grok_usage;
mod path_norm;
mod platform;
mod proxy;
mod pty;
mod state;
mod tools;
mod window_chrome;

use chrono::Utc;
use path_norm::folder_name;
use proxy::validate_proxy_url;
use pty::{LivePtyInfo, PtyHub, PtyOpened};
use serde::{Deserialize, Serialize};
use state::{AppSettings, AppState, Project, overlay_key};
use std::path::Path;
use std::sync::Arc;
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, State};
use grok_accounts::GrokAccountList;
use grok_usage::GrokUsage;
use tools::update::{ToolVersionInfo, UpgradeResult};
use tools::{BinaryProbe, SessionRow, ToolId};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDraft {
    name: String,
    path: String,
    proxy_enabled: bool,
    proxy_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectUpdate {
    id: String,
    name: String,
    path: String,
    proxy_enabled: bool,
    proxy_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionListResult {
    ok: bool,
    sessions: Vec<SessionRow>,
    tool_found: bool,
    tool_path: Option<String>,
    error_kind: Option<String>,
    message: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RenameResult {
    kind: tools::RenameKind,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolProbeMap {
    opencode: BinaryProbe,
    grokbuild: BinaryProbe,
    kimi: BinaryProbe,
}

fn validate_project_fields(name: &str, path: &str, proxy_enabled: bool, proxy_url: &str) -> Result<(String, String, String), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("项目名称不能为空".into());
    }
    let path = path.trim();
    if path.is_empty() {
        return Err("请选择项目文件夹".into());
    }
    if !Path::new(path).is_dir() {
        return Err(format!("文件夹不存在或无法读取：{path}。请重新选择。"));
    }
    let proxy_url = if proxy_enabled {
        validate_proxy_url(proxy_url)?
    } else if proxy_url.trim().is_empty() {
        crate::proxy::default_proxy_url()
    } else {
        validate_proxy_url(proxy_url).unwrap_or_else(|_| proxy_url.trim().to_string())
    };
    Ok((name.to_string(), path.to_string(), proxy_url))
}

#[tauri::command]
fn load_app_state(app: AppHandle) -> Result<AppState, String> {
    state::load_state(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: AppSettings) -> Result<AppState, String> {
    let mut next = settings;
    if next.terminal_font_size < 10 || next.terminal_font_size > 22 {
        next.terminal_font_size = 13;
    }
    next.default_proxy_url = validate_proxy_url(&next.default_proxy_url)?;
    if next.powershell_path.trim().is_empty()
        || (!cfg!(windows) && platform::is_windows_shell_name(&next.powershell_path))
    {
        next.powershell_path = platform::default_shell();
    }
    let mut state = state::load_state(&app)?;
    state.settings = next;
    state::save_state(&app, &state)?;
    Ok(state)
}

#[tauri::command]
fn add_project(app: AppHandle, draft: ProjectDraft) -> Result<AppState, String> {
    let (name, path, proxy_url) =
        validate_project_fields(&draft.name, &draft.path, draft.proxy_enabled, &draft.proxy_url)?;
    let mut state = state::load_state(&app)?;
    if state.projects.iter().any(|item| path_norm::paths_equal(&item.path, &path)) {
        return Err("这个文件夹已经在项目列表里".into());
    }
    state.projects.insert(
        0,
        Project {
            id: Uuid::new_v4().to_string(),
            name,
            path,
            proxy_enabled: draft.proxy_enabled,
            proxy_url,
            created_at: Utc::now().to_rfc3339(),
        },
    );
    state::save_state(&app, &state)?;
    Ok(state)
}

#[tauri::command]
fn update_project(app: AppHandle, draft: ProjectUpdate) -> Result<AppState, String> {
    let (name, path, proxy_url) =
        validate_project_fields(&draft.name, &draft.path, draft.proxy_enabled, &draft.proxy_url)?;
    let mut state = state::load_state(&app)?;
    let project = state
        .projects
        .iter_mut()
        .find(|item| item.id == draft.id)
        .ok_or_else(|| "项目不存在".to_string())?;
    project.name = name;
    project.path = path;
    project.proxy_enabled = draft.proxy_enabled;
    project.proxy_url = proxy_url;
    state::save_state(&app, &state)?;
    Ok(state)
}

#[tauri::command]
fn remove_project(app: AppHandle, project_id: String) -> Result<AppState, String> {
    let mut state = state::load_state(&app)?;
    let before = state.projects.len();
    state.projects.retain(|item| item.id != project_id);
    if state.projects.len() == before {
        return Err("项目不存在".into());
    }
    state::save_state(&app, &state)?;
    Ok(state)
}

#[tauri::command]
fn folder_label(path: String) -> String {
    folder_name(&path)
}

#[tauri::command]
fn app_version() -> String {
    tools::update::app_version()
}

#[tauri::command]
fn host_platform() -> &'static str {
    platform::host_platform()
}

#[tauri::command]
async fn list_tool_versions(app: AppHandle) -> Result<Vec<ToolVersionInfo>, String> {
    let state = state::load_state(&app)?;
    tauri::async_runtime::spawn_blocking(move || tools::update::list_versions(&state.settings))
        .await
        .map_err(|err| format!("检查版本失败：{err}"))
}

#[tauri::command]
fn list_grok_accounts(app: AppHandle) -> Result<GrokAccountList, String> {
    grok_accounts::list_accounts(&app)
}

#[tauri::command]
fn save_grok_account(app: AppHandle, name: String) -> Result<GrokAccountList, String> {
    grok_accounts::save_current(&app, name)
}

#[tauri::command]
fn switch_grok_account(app: AppHandle, account_id: String) -> Result<GrokAccountList, String> {
    grok_accounts::switch_account(&app, &account_id)
}

#[tauri::command]
fn delete_grok_account(app: AppHandle, account_id: String) -> Result<GrokAccountList, String> {
    grok_accounts::delete_account(&app, &account_id)
}

#[tauri::command]
async fn grok_usage(app: AppHandle, project_id: Option<String>) -> Result<GrokUsage, String> {
    tauri::async_runtime::spawn_blocking(move || crate::grok_usage::fetch(&app, project_id.as_deref()))
        .await
        .map_err(|err| format!("读取 Grok 用量失败：{err}"))
}

#[tauri::command]
async fn login_grok_account(app: AppHandle) -> Result<GrokAccountList, String> {
    let state = state::load_state(&app)?;
    tauri::async_runtime::spawn_blocking(move || grok_accounts::login_new(&app, &state.settings))
        .await
        .map_err(|err| format!("登录失败：{err}"))?
}

#[tauri::command]
async fn upgrade_tool(app: AppHandle, tool_id: ToolId) -> Result<UpgradeResult, String> {
    let state = state::load_state(&app)?;
    tauri::async_runtime::spawn_blocking(move || tools::update::upgrade_tool(tool_id, &state.settings))
        .await
        .map_err(|err| format!("升级失败：{err}"))
}

#[tauri::command]
fn reveal_main_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        window_chrome::reveal(&win);
    }
}

#[tauri::command]
async fn probe_tools(app: AppHandle) -> Result<ToolProbeMap, String> {
    let state = state::load_state(&app)?;
    tauri::async_runtime::spawn_blocking(move || ToolProbeMap {
        opencode: tools::probe_binary(ToolId::Opencode, &state.settings),
        grokbuild: tools::probe_binary(ToolId::Grokbuild, &state.settings),
        kimi: tools::probe_binary(ToolId::Kimi, &state.settings),
    })
    .await
    .map_err(|err| format!("探测工具失败：{err}"))
}

#[tauri::command]
async fn list_sessions(app: AppHandle, project_id: String, tool_id: ToolId) -> Result<SessionListResult, String> {
    tauri::async_runtime::spawn_blocking(move || list_sessions_inner(&app, project_id, tool_id))
        .await
        .map_err(|err| format!("加载会话失败：{err}"))?
}

fn list_sessions_inner(app: &AppHandle, project_id: String, tool_id: ToolId) -> Result<SessionListResult, String> {
    let state = state::load_state(app)?;
    let project = state::find_project(&state, &project_id)?;
    if !Path::new(&project.path).is_dir() {
        return Ok(SessionListResult {
            ok: false,
            sessions: Vec::new(),
            tool_found: false,
            tool_path: None,
            error_kind: Some("path_unreadable".into()),
            message: Some(format!(
                "项目文件夹读不了：{}。请重新选择文件夹，或从列表移除这个项目。",
                project.path
            )),
        });
    }
    let probe = tools::probe_binary(tool_id, &state.settings);
    if !probe.found {
        return Ok(SessionListResult {
            ok: false,
            sessions: Vec::new(),
            tool_found: false,
            tool_path: None,
            error_kind: Some("cli_missing".into()),
            message: probe.message,
        });
    }
    match tools::list_sessions(tool_id, &project.path, &state.settings) {
        Ok(mut sessions) => {
            for row in &mut sessions {
                if let Some(overlay) = state.title_overlays.get(&overlay_key(tool_id.as_str(), &row.id)) {
                    row.title = overlay.clone();
                }
            }
            Ok(SessionListResult {
                ok: true,
                sessions,
                tool_found: true,
                tool_path: probe.path,
                error_kind: None,
                message: None,
            })
        }
        Err(message) => Ok(SessionListResult {
            ok: false,
            sessions: Vec::new(),
            tool_found: true,
            tool_path: probe.path,
            error_kind: Some("scan_failed".into()),
            message: Some(message),
        }),
    }
}

#[tauri::command]
fn rename_session(
    app: AppHandle,
    project_id: String,
    tool_id: ToolId,
    session_id: String,
    title: String,
) -> Result<RenameResult, String> {
    let mut state = state::load_state(&app)?;
    let project = state::find_project(&state, &project_id)?.clone();
    let kind = tools::rename_session(tool_id, &project.path, &session_id, &title, &state.settings)?;
    if kind == tools::RenameKind::Overlay {
        state
            .title_overlays
            .insert(overlay_key(tool_id.as_str(), &session_id), title.trim().to_string());
        state::save_state(&app, &state)?;
    }
    Ok(RenameResult { kind })
}

#[tauri::command]
fn delete_session(
    app: AppHandle,
    project_id: String,
    tool_id: ToolId,
    session_id: String,
) -> Result<(), String> {
    let mut state = state::load_state(&app)?;
    let project = state::find_project(&state, &project_id)?.clone();
    tools::delete_session(tool_id, &project.path, &session_id, &state.settings)?;
    if state
        .title_overlays
        .remove(&overlay_key(tool_id.as_str(), &session_id))
        .is_some()
    {
        state::save_state(&app, &state)?;
    }
    Ok(())
}

#[tauri::command]
fn pty_open(
    app: AppHandle,
    hub: State<Arc<PtyHub>>,
    project_id: String,
    tool_id: ToolId,
    session_id: Option<String>,
    title: String,
    cols: u16,
    rows: u16,
) -> Result<PtyOpened, String> {
    let state = state::load_state(&app)?;
    hub.open(app, &state, &project_id, tool_id, session_id, title, cols, rows)
}

#[tauri::command]
fn pty_write(hub: State<Arc<PtyHub>>, pty_id: String, data: String) -> Result<(), String> {
    hub.write(&pty_id, &data)
}

#[tauri::command]
fn pty_resize(hub: State<Arc<PtyHub>>, pty_id: String, cols: u16, rows: u16) -> Result<(), String> {
    hub.resize(&pty_id, cols, rows)
}

#[tauri::command]
fn pty_kill(hub: State<Arc<PtyHub>>, pty_id: String) -> Result<(), String> {
    hub.kill(&pty_id)
}

#[tauri::command]
fn list_live_ptys(hub: State<Arc<PtyHub>>) -> Vec<LivePtyInfo> {
    hub.list()
}

#[tauri::command]
async fn codex_probe(
    app: AppHandle,
    hub: State<'_, Arc<bridge::BridgeHub>>,
    project_id: String,
) -> Result<bridge::CodexProbe, String> {
    let hub = hub.inner().clone();
    tauri::async_runtime::spawn_blocking(move || Ok(hub.probe(&app, &project_id)))
        .await
        .map_err(|err| format!("探测 Codex 失败：{err}"))?
}

#[tauri::command]
async fn list_codex_threads(
    app: AppHandle,
    hub: State<'_, Arc<bridge::BridgeHub>>,
    project_id: String,
) -> Result<bridge::CodexThreadList, String> {
    let hub = hub.inner().clone();
    tauri::async_runtime::spawn_blocking(move || hub.list_threads(&app, &project_id))
        .await
        .map_err(|err| format!("列出 Codex 线程失败：{err}"))?
}

#[tauri::command]
async fn git_snapshot(
    app: AppHandle,
    hub: State<'_, Arc<bridge::BridgeHub>>,
    project_id: String,
) -> Result<bridge::GitSnapshot, String> {
    let hub = hub.inner().clone();
    tauri::async_runtime::spawn_blocking(move || hub.git_snapshot(&app, &project_id))
        .await
        .map_err(|err| format!("读取 git 状态失败：{err}"))?
}

#[tauri::command]
async fn start_codex_review(
    app: AppHandle,
    hub: State<'_, Arc<bridge::BridgeHub>>,
    project_id: String,
    thread_ids: Vec<String>,
    target_kind: String,
    commit_sha: Option<String>,
    base_branch: Option<String>,
    custom_instructions: Option<String>,
) -> Result<bridge::ReviewStartResult, String> {
    let hub = hub.inner().clone();
    let req = bridge::ReviewRequest {
        project_id,
        thread_ids,
        target_kind,
        commit_sha,
        base_branch,
        custom_instructions,
    };
    tauri::async_runtime::spawn_blocking(move || hub.start_review(&app, req))
        .await
        .map_err(|err| format!("启动审查失败：{err}"))?
}

#[tauri::command]
async fn cursor_dev_run(
    app: AppHandle,
    project_id: String,
    prompt: String,
    agent_id: Option<String>,
) -> Result<cursor_dev::CursorDevResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        cursor_dev::run(
            &app,
            cursor_dev::CursorDevRequest {
                project_id,
                prompt,
                agent_id,
            },
        )
    })
    .await
    .map_err(|err| format!("启动 Cursor 开发失败：{err}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    platform::apply_process_path();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_page_load(|webview, payload| {
            if payload.event() != PageLoadEvent::Finished {
                return;
            }
            if let Some(win) = webview.app_handle().get_webview_window(webview.label()) {
                let _ = win.show();
            }
        })
        .setup(|app| {
            let hub = PtyHub::new();
            hub.start_monitor(app.handle().clone());
            app.manage(hub);
            app.manage(Arc::new(bridge::BridgeHub::new()));
            if let Some(win) = app.get_webview_window("main") {
                #[cfg(windows)]
                {
                    let _ = win.set_shadow(false);
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = win.set_shadow(true);
                    let _ = win.set_title_bar_style(tauri::TitleBarStyle::Overlay);
                }
                #[cfg(all(not(windows), not(target_os = "macos")))]
                {
                    let _ = win.set_shadow(true);
                }
                window_chrome::attach(&win);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_app_state,
            save_settings,
            add_project,
            update_project,
            remove_project,
            folder_label,
            reveal_main_window,
            probe_tools,
            app_version,
            host_platform,
            list_tool_versions,
            upgrade_tool,
            list_grok_accounts,
            grok_usage,
            save_grok_account,
            switch_grok_account,
            delete_grok_account,
            login_grok_account,
            list_sessions,
            rename_session,
            delete_session,
            pty_open,
            pty_write,
            pty_resize,
            pty_kill,
            list_live_ptys,
            codex_probe,
            list_codex_threads,
            git_snapshot,
            start_codex_review,
            cursor_dev_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running Agent Dock");
}
