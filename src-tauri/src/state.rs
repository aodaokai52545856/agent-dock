use crate::proxy::default_proxy_url;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub proxy_enabled: bool,
    pub proxy_url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub default_proxy_url: String,
    pub opencode_path: String,
    pub grokbuild_path: String,
    pub kimi_path: String,
    #[serde(default = "crate::platform::default_shell")]
    pub powershell_path: String,
    pub terminal_font_size: u32,
    #[serde(default = "default_ui_font_size")]
    pub ui_font_size: u32,
    #[serde(default = "default_ui_theme")]
    pub ui_theme: String,
    #[serde(default)]
    pub ui_accent: String,
    #[serde(default)]
    pub ui_background: String,
    #[serde(default)]
    pub ui_foreground: String,
    #[serde(default)]
    pub ui_font_family: String,
    #[serde(default)]
    pub content_font_family: String,
    #[serde(default)]
    pub code_font_family: String,
    #[serde(default = "default_ui_contrast")]
    pub ui_contrast: u32,
    #[serde(default = "default_translucent_sidebar")]
    pub translucent_sidebar: bool,
    #[serde(default = "default_ui_opacity")]
    pub ui_opacity: u32,
    #[serde(default = "default_session_tool_filter")]
    pub session_tool_filter: String,
    #[serde(default)]
    pub cursor_api_key: String,
    #[serde(default)]
    pub codex_path: String,
}

fn default_ui_opacity() -> u32 {
    0
}

fn default_ui_font_size() -> u32 {
    13
}

fn default_ui_theme() -> String {
    "system".into()
}

fn default_ui_contrast() -> u32 {
    60
}

fn default_translucent_sidebar() -> bool {
    true
}

fn normalize_ui_theme(value: &str) -> String {
    match value {
        "light" | "dark" | "system" => value.to_string(),
        _ => default_ui_theme(),
    }
}

fn default_session_tool_filter() -> String {
    "all".into()
}

fn normalize_session_tool_filter(value: &str) -> String {
    match value {
        "all" | "opencode" | "grokbuild" | "kimi" => value.to_string(),
        _ => default_session_tool_filter(),
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_proxy_url: default_proxy_url(),
            opencode_path: String::new(),
            grokbuild_path: String::new(),
            kimi_path: String::new(),
            powershell_path: crate::platform::default_shell(),
            terminal_font_size: 13,
            ui_font_size: default_ui_font_size(),
            ui_theme: default_ui_theme(),
            ui_accent: String::new(),
            ui_background: String::new(),
            ui_foreground: String::new(),
            ui_font_family: String::new(),
            content_font_family: String::new(),
            code_font_family: String::new(),
            ui_contrast: default_ui_contrast(),
            translucent_sidebar: default_translucent_sidebar(),
            ui_opacity: default_ui_opacity(),
            session_tool_filter: default_session_tool_filter(),
            cursor_api_key: String::new(),
            codex_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub projects: Vec<Project>,
    pub settings: AppSettings,
    #[serde(default)]
    pub title_overlays: BTreeMap<String, String>,
}

pub fn overlay_key(tool_id: &str, session_id: &str) -> String {
    format!("{tool_id}:{session_id}")
}

pub fn state_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("无法定位应用数据目录：{err}"))?;
    fs::create_dir_all(&dir).map_err(|err| format!("无法创建应用数据目录：{err}"))?;
    Ok(dir.join("state.json"))
}

pub fn load_state(app: &AppHandle) -> Result<AppState, String> {
    let path = state_path(app)?;
    if !path.exists() {
        return Ok(AppState {
            settings: AppSettings::default(),
            ..AppState::default()
        });
    }
    let text = fs::read_to_string(&path).map_err(|err| format!("读取配置失败：{err}"))?;
    let mut state: AppState =
        serde_json::from_str(&text).map_err(|err| format!("配置文件损坏，请检查 state.json：{err}"))?;
    if state.settings.default_proxy_url.trim().is_empty() {
        state.settings.default_proxy_url = default_proxy_url();
    }
    if state.settings.powershell_path.trim().is_empty()
        || (!cfg!(windows) && crate::platform::is_windows_shell_name(&state.settings.powershell_path))
    {
        state.settings.powershell_path = crate::platform::default_shell();
    }
    if state.settings.terminal_font_size < 10 || state.settings.terminal_font_size > 22 {
        state.settings.terminal_font_size = 13;
    }
    let mut dirty = false;
    if state.settings.ui_font_size < 11 || state.settings.ui_font_size > 18 {
        state.settings.ui_font_size = default_ui_font_size();
        dirty = true;
    }
    if state.settings.ui_contrast > 100 {
        state.settings.ui_contrast = default_ui_contrast();
        dirty = true;
    }
    let theme = normalize_ui_theme(&state.settings.ui_theme);
    if theme != state.settings.ui_theme {
        state.settings.ui_theme = theme;
        dirty = true;
    }
    if state.settings.ui_opacity > 80
        || state.settings.ui_opacity == 40
        || state.settings.ui_opacity == 10
    {
        state.settings.ui_opacity = default_ui_opacity();
        dirty = true;
    }
    state.settings.session_tool_filter = normalize_session_tool_filter(&state.settings.session_tool_filter);
    if dirty {
        let _ = save_state(app, &state);
    }
    Ok(state)
}

pub fn save_state(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let path = state_path(app)?;
    let text = serde_json::to_string_pretty(state).map_err(|err| format!("序列化配置失败：{err}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|err| format!("写入配置失败：{err}"))?;
    fs::rename(&tmp, &path).map_err(|err| format!("保存配置失败：{err}"))?;
    Ok(())
}

pub fn find_project<'a>(state: &'a AppState, project_id: &str) -> Result<&'a Project, String> {
    state
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .ok_or_else(|| "项目不存在，请重新选择左侧项目".into())
}
