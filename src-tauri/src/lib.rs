mod bridge;
mod clipboard;
mod ccswitch;
mod cursor_dev;
mod dsh_balance;
mod dsh_credentials;
mod dsh_embed;
mod dsh_keys;
mod dsh_web;
mod dsh_web_guard;
mod grok_accounts;
mod secret_box;
mod grok_spend;
mod grok_usage;
mod path_norm;
mod platform;
mod proxy;
mod pty;
mod session_cite;
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
use tauri::{AppHandle, Emitter, Manager, State};
use dsh_balance::DshBalance;
use grok_accounts::GrokAccountList;
use grok_spend::GrokSpend;
use grok_usage::GrokUsage;
use session_cite::{SessionDoc, SessionDocBody, SessionTurn};
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
    claude: BinaryProbe,
    pi: BinaryProbe,
    dsh: BinaryProbe,
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
        next.terminal_font_size = state::default_terminal_font_size();
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
fn clipboard_write(text: String) -> Result<(), String> {
    clipboard::write_text(&text)
}

#[tauri::command]
fn clipboard_read() -> Result<String, String> {
    clipboard::read_text()
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
async fn list_tool_versions(
    app: AppHandle,
    proxy_url: Option<String>,
    tool_id: Option<ToolId>,
) -> Result<Vec<ToolVersionInfo>, String> {
    let state = state::load_state(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        tools::update::list_versions(&state.settings, proxy_url.as_deref(), tool_id)
    })
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
async fn grok_spend(start: Option<i64>, end: Option<i64>) -> Result<GrokSpend, String> {
    tauri::async_runtime::spawn_blocking(move || crate::grok_spend::fetch(start, end))
        .await
        .map_err(|err| format!("读取 Grok token 消耗失败：{err}"))
}

#[tauri::command]
async fn list_session_docs(
    app: AppHandle,
    project_id: String,
    tool_id: ToolId,
    session_id: String,
) -> Result<Vec<SessionDoc>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        session_cite::list_session_docs(&app, &project_id, tool_id, &session_id)
    })
    .await
    .map_err(|err| format!("列出会话文档失败：{err}"))?
}

#[tauri::command]
async fn read_session_doc(app: AppHandle, project_id: String, path: String) -> Result<SessionDocBody, String> {
    tauri::async_runtime::spawn_blocking(move || session_cite::read_session_doc(&app, &project_id, &path))
        .await
        .map_err(|err| format!("读取文档失败：{err}"))?
}

#[tauri::command]
async fn list_session_turns(
    app: AppHandle,
    project_id: String,
    tool_id: ToolId,
    session_id: String,
) -> Result<Vec<SessionTurn>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        session_cite::list_session_turns(&app, &project_id, tool_id, &session_id)
    })
    .await
    .map_err(|err| format!("读取会话片段失败：{err}"))?
}

#[tauri::command]
async fn login_grok_account(app: AppHandle) -> Result<GrokAccountList, String> {
    let state = state::load_state(&app)?;
    tauri::async_runtime::spawn_blocking(move || grok_accounts::login_new(&app, &state.settings))
        .await
        .map_err(|err| format!("登录失败：{err}"))?
}

#[tauri::command]
async fn upgrade_tool(
    app: AppHandle,
    tool_id: ToolId,
    proxy_url: Option<String>,
) -> Result<UpgradeResult, String> {
    let state = state::load_state(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        tools::update::upgrade_tool(tool_id, &state.settings, proxy_url.as_deref())
    })
    .await
    .map_err(|err| format!("升级失败：{err}"))
}

#[tauri::command]
async fn uninstall_tool(app: AppHandle, tool_id: ToolId) -> Result<UpgradeResult, String> {
    let state = state::load_state(&app)?;
    tauri::async_runtime::spawn_blocking(move || tools::update::uninstall_tool(tool_id, &state.settings))
        .await
        .map_err(|err| format!("卸载失败：{err}"))
}

#[tauri::command]
fn probe_ccswitch(app: AppHandle) -> Result<ccswitch::Probe, String> {
    let state = state::load_state(&app)?;
    Ok(ccswitch::probe(&state.settings))
}

#[tauri::command]
async fn ccswitch_latest(proxy_url: Option<String>) -> Result<ccswitch::Latest, String> {
    tauri::async_runtime::spawn_blocking(move || {
        ccswitch::fetch_latest(proxy_url.as_deref().filter(|item| !item.trim().is_empty()))
    })
    .await
    .map_err(|err| format!("查询 CC Switch 最新版失败：{err}"))?
}

#[tauri::command]
async fn install_ccswitch(
    app: AppHandle,
    download_dir: String,
    install_dir: String,
    proxy_url: Option<String>,
) -> Result<ccswitch::InstallResult, String> {
    let state = state::load_state(&app)?;
    let handle = app.clone();
    let (result, settings) = tauri::async_runtime::spawn_blocking(move || {
        let mut settings = state.settings;
        let result = ccswitch::install(
            &mut settings,
            &download_dir,
            &install_dir,
            proxy_url.as_deref().filter(|item| !item.trim().is_empty()),
            |progress| {
                let _ = handle.emit("ccswitch-progress", &progress);
            },
        )?;
        Ok::<_, String>((result, settings))
    })
    .await
    .map_err(|err| format!("安装 CC Switch 失败：{err}"))??;
    let mut next = state::load_state(&app)?;
    next.settings.ccswitch_path = settings.ccswitch_path;
    next.settings.ccswitch_download_dir = settings.ccswitch_download_dir;
    next.settings.ccswitch_install_dir = settings.ccswitch_install_dir;
    state::save_state(&app, &next)?;
    Ok(result)
}

#[tauri::command]
fn launch_ccswitch(app: AppHandle, path: Option<String>) -> Result<(), String> {
    let state = state::load_state(&app)?;
    let target = path
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .or_else(|| ccswitch::probe(&state.settings).path)
        .ok_or_else(|| "未安装 CC Switch，请先下载安装。".to_string())?;
    ccswitch::launch(&target)
}

#[tauri::command]
fn dsh_key_status(project_path: Option<String>) -> dsh_credentials::DshKeyStatus {
    dsh_credentials::live_status(project_path.as_deref().map(Path::new))
}

#[tauri::command]
fn dsh_list_keys(app: AppHandle, project_path: Option<String>) -> Result<dsh_keys::DshKeyBundle, String> {
    dsh_keys::list(&app, project_path.as_deref().map(Path::new))
}

#[tauri::command]
fn dsh_add_key(
    app: AppHandle,
    name: String,
    key: String,
    project_path: Option<String>,
) -> Result<dsh_keys::DshKeyBundle, String> {
    dsh_keys::add(&app, &name, &key, project_path.as_deref().map(Path::new))
}

#[tauri::command]
fn dsh_switch_key(
    app: AppHandle,
    id: String,
    project_path: Option<String>,
) -> Result<dsh_keys::DshKeyBundle, String> {
    dsh_keys::switch_to(&app, &id, project_path.as_deref().map(Path::new))
}

#[tauri::command]
fn dsh_delete_key(
    app: AppHandle,
    id: String,
    project_path: Option<String>,
) -> Result<dsh_keys::DshKeyBundle, String> {
    dsh_keys::delete(&app, &id, project_path.as_deref().map(Path::new))
}

#[tauri::command]
fn dsh_rename_key(
    app: AppHandle,
    id: String,
    name: String,
    project_path: Option<String>,
) -> Result<dsh_keys::DshKeyBundle, String> {
    dsh_keys::rename(&app, &id, &name, project_path.as_deref().map(Path::new))
}

#[tauri::command]
async fn dsh_balance(app: AppHandle, project_id: Option<String>) -> Result<DshBalance, String> {
    tauri::async_runtime::spawn_blocking(move || crate::dsh_balance::fetch(&app, project_id.as_deref()))
        .await
        .map_err(|err| format!("读取 DeepSeek 余额失败：{err}"))
}

#[tauri::command]
fn remember_ccswitch_path(app: AppHandle, path: String) -> Result<AppState, String> {
    let mut state = state::load_state(&app)?;
    ccswitch::remember_path(&mut state.settings, &path)?;
    state::save_state(&app, &state)?;
    Ok(state)
}

#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("只打开 http(s) 链接".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
            .map_err(|err| format!("无法打开链接：{err}"))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|err| format!("无法打开链接：{err}"))?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|err| format!("无法打开链接：{err}"))?;
        return Ok(());
    }
}

fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        return;
    }
    if let Some(webview) = app.get_webview("main") {
        let _ = webview.window().show();
    }
}

#[tauri::command]
fn reveal_main_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        window_chrome::reveal(&win);
        return;
    }
    if let Some(webview) = app.get_webview("main") {
        let win = webview.window();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[tauri::command]
fn set_window_frost(app: AppHandle, frost: u32) {
    if let Some(win) = app.get_webview_window("main") {
        window_chrome::set_frost(&win, frost);
    }
}

#[tauri::command]
async fn probe_tools(app: AppHandle) -> Result<ToolProbeMap, String> {
    let state = state::load_state(&app)?;
    tauri::async_runtime::spawn_blocking(move || ToolProbeMap {
        opencode: tools::probe_binary(ToolId::Opencode, &state.settings),
        grokbuild: tools::probe_binary(ToolId::Grokbuild, &state.settings),
        kimi: tools::probe_binary(ToolId::Kimi, &state.settings),
        claude: tools::probe_binary(ToolId::Claude, &state.settings),
        pi: tools::probe_binary(ToolId::Pi, &state.settings),
        dsh: tools::probe_binary(ToolId::Dsh, &state.settings),
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
    match tools::list_sessions(tool_id, &project.path, &state.settings) {
        Ok(mut sessions) => {
            for row in &mut sessions {
                if tool_id == ToolId::Dsh {
                    continue;
                }
                if let Some(overlay) = state::overlay_title(&state.title_overlays, tool_id.as_str(), &row.id)
                {
                    row.title = overlay.to_string();
                }
            }
            if sessions.is_empty() && !probe.found {
                return Ok(SessionListResult {
                    ok: false,
                    sessions: Vec::new(),
                    tool_found: false,
                    tool_path: probe.path,
                    error_kind: Some("cli_missing".into()),
                    message: probe.message,
                });
            }
            Ok(SessionListResult {
                ok: true,
                sessions,
                tool_found: probe.found,
                tool_path: probe.path,
                error_kind: None,
                message: if probe.found { None } else { probe.message },
            })
        }
        Err(message) => Ok(SessionListResult {
            ok: false,
            sessions: Vec::new(),
            tool_found: probe.found,
            tool_path: probe.path,
            error_kind: Some(if probe.found {
                "scan_failed".into()
            } else {
                "cli_missing".into()
            }),
            message: Some(if probe.found {
                message
            } else {
                probe.message.unwrap_or(message)
            }),
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
    state::put_title_overlay(&mut state, tool_id.as_str(), &session_id, &title);
    state::save_state(&app, &state)?;
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
async fn pty_open(
    app: AppHandle,
    hub: State<'_, Arc<PtyHub>>,
    web: State<'_, Arc<dsh_web::Hub>>,
    project_id: String,
    tool_id: ToolId,
    session_id: Option<String>,
    title: String,
    cols: u16,
    rows: u16,
    ui_theme: Option<String>,
) -> Result<PtyOpened, String> {
    if tool_id == ToolId::Dsh {
        let state = state::load_state(&app)?;
        let web = web.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || {
            web.open(&app, &state, &project_id, session_id, title)
        })
        .await
        .map_err(|err| format!("启动 DeepSeek Web 失败：{err}"))?;
    }
    let state = state::load_state(&app)?;
    hub.open(
        app,
        &state,
        &project_id,
        tool_id,
        session_id,
        title,
        cols,
        rows,
        ui_theme.as_deref(),
    )
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
fn pty_kill(
    app: AppHandle,
    hub: State<Arc<PtyHub>>,
    web: State<Arc<dsh_web::Hub>>,
    pty_id: String,
) -> Result<(), String> {
    if web.kill(&pty_id)? {
        let _ = dsh_embed::close(&app);
        return Ok(());
    }
    hub.kill(&pty_id)
}

#[tauri::command]
async fn dsh_embed_open(
    app: AppHandle,
    url: String,
    bounds: dsh_embed::Bounds,
    theme_script: Option<String>,
) -> Result<(), String> {
    dsh_embed::open(&app, url, bounds, theme_script)
}

#[tauri::command]
async fn dsh_embed_set_bounds(app: AppHandle, bounds: dsh_embed::Bounds) -> Result<(), String> {
    dsh_embed::set_bounds(&app, bounds)
}

#[tauri::command]
async fn dsh_embed_set_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    dsh_embed::set_visible(&app, visible)
}

#[tauri::command]
async fn dsh_embed_close(app: AppHandle) -> Result<(), String> {
    dsh_embed::close(&app)
}

#[tauri::command]
async fn dsh_embed_apply_theme(app: AppHandle, script: String) -> Result<(), String> {
    dsh_embed::apply_theme(&app, script)
}

#[tauri::command]
fn pty_bind_session(
    hub: State<Arc<PtyHub>>,
    web: State<Arc<dsh_web::Hub>>,
    pty_id: String,
    session_id: String,
    title: Option<String>,
) -> Result<LivePtyInfo, String> {
    if let Some(info) = web.bind_session(&pty_id, &session_id, title.clone())? {
        return Ok(info);
    }
    hub.bind_session(&pty_id, &session_id, title)
}

#[tauri::command]
fn list_live_ptys(hub: State<Arc<PtyHub>>, web: State<Arc<dsh_web::Hub>>) -> Vec<LivePtyInfo> {
    let mut live = hub.list();
    live.extend(web.list());
    live
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
            if webview.label() == "dsh-embed" {
                dsh_embed::on_loaded(&webview);
                return;
            }
            show_main_window(webview.app_handle());
        })
        .setup(|app| {
            let hub = PtyHub::new();
            hub.start_monitor(app.handle().clone());
            app.manage(hub);
            app.manage(Arc::new(dsh_embed::Hub::default()));
            app.manage(Arc::new(bridge::BridgeHub::new()));
            let web = dsh_web::Hub::new();
            if let Ok(dir) = app.path().app_data_dir() {
                web.set_lock_path(dir.join("dsh-web-pids.json"));
            }
            web.reap_strays();
            app.manage(web);
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
                let closer = app.handle().clone();
                window_chrome::attach(&win, move || dsh_web::shutdown_app(&closer));
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
            clipboard_write,
            clipboard_read,
            reveal_main_window,
            set_window_frost,
            probe_tools,
            app_version,
            host_platform,
            list_tool_versions,
            upgrade_tool,
            uninstall_tool,
            probe_ccswitch,
            ccswitch_latest,
            install_ccswitch,
            launch_ccswitch,
            remember_ccswitch_path,
            dsh_key_status,
            dsh_list_keys,
            dsh_add_key,
            dsh_switch_key,
            dsh_delete_key,
            dsh_rename_key,
            dsh_balance,
            open_external,
            list_grok_accounts,
            grok_usage,
            grok_spend,
            save_grok_account,
            switch_grok_account,
            delete_grok_account,
            login_grok_account,
            list_sessions,
            list_session_docs,
            read_session_doc,
            list_session_turns,
            rename_session,
            delete_session,
            pty_open,
            pty_write,
            pty_resize,
            pty_kill,
            dsh_embed_open,
            dsh_embed_set_bounds,
            dsh_embed_set_visible,
            dsh_embed_close,
            dsh_embed_apply_theme,
            pty_bind_session,
            list_live_ptys,
            codex_probe,
            list_codex_threads,
            git_snapshot,
            start_codex_review,
            cursor_dev_run
        ])
        .build(tauri::generate_context!())
        .expect("error while building Agent Dock")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                dsh_web::shutdown_app(app);
            }
        });
}
