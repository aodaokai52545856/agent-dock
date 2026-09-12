use serde::Deserialize;
use std::sync::Mutex;
use tauri::webview::WebviewBuilder;
use tauri::window::Color;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};

const LABEL: &str = "dsh-embed";

#[derive(Default)]
pub struct Hub {
    origin: Mutex<Option<String>>,
    theme_script: Mutex<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn origin_key(url: &str) -> String {
    let without_hash = url.split('#').next().unwrap_or(url);
    let without_query = without_hash.split('?').next().unwrap_or(without_hash);
    without_query.trim_end_matches('/').to_ascii_lowercase()
}

pub fn glass_init_script(theme_script: Option<&str>) -> String {
    const BOOT: &str = include_str!("dsh_glass.js");
    match theme_script {
        Some(theme) if !theme.trim().is_empty() => format!("{BOOT}\n{theme}"),
        _ => BOOT.to_string(),
    }
}

pub fn open(
    app: &AppHandle,
    url: String,
    bounds: Bounds,
    theme_script: Option<String>,
) -> Result<(), String> {
    let parsed = url
        .parse()
        .map_err(|err| format!("DeepSeek 页面地址无效：{err}"))?;
    let origin = origin_key(&url);
    if let Some(script) = theme_script.as_deref() {
        store_theme(app, Some(script));
    }
    let existing = app.get_webview(LABEL);
    let same = existing
        .as_ref()
        .and_then(|_| app.try_state::<std::sync::Arc<Hub>>())
        .and_then(|hub| hub.origin.lock().ok().and_then(|guard| guard.clone()))
        .is_some_and(|current| current == origin);

    if let Some(webview) = existing {
        if same {
            apply_bounds(&webview, &bounds)?;
            let _ = webview.show();
            paint(&webview, stored_theme(app).as_deref());
            return Ok(());
        }
        let _ = webview.close();
        if let Some(hub) = app.try_state::<std::sync::Arc<Hub>>() {
            if let Ok(mut guard) = hub.origin.lock() {
                *guard = None;
            }
        }
    }

    let host = app
        .get_webview_window("main")
        .ok_or_else(|| "没有主窗口，无法嵌入 DeepSeek Web。".to_string())?;
    let window = host.as_ref().window();
    let init = glass_init_script(theme_script.as_deref());
    let builder = WebviewBuilder::new(LABEL, WebviewUrl::External(parsed))
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        .initialization_script_for_all_frames(init);
    window
        .add_child(
            builder,
            LogicalPosition::new(bounds.x, bounds.y),
            LogicalSize::new(bounds.width.max(1.0), bounds.height.max(1.0)),
        )
        .map_err(|err| format!("无法嵌入 DeepSeek Web：{err}"))?;
    if let Some(webview) = app.get_webview(LABEL) {
        paint(&webview, stored_theme(app).as_deref());
        raise_parent_resize_overlay(&webview);
    }
    if let Some(hub) = app.try_state::<std::sync::Arc<Hub>>() {
        if let Ok(mut guard) = hub.origin.lock() {
            *guard = Some(origin);
        }
    }
    Ok(())
}

pub fn set_bounds(app: &AppHandle, bounds: Bounds) -> Result<(), String> {
    let Some(webview) = app.get_webview(LABEL) else {
        return Ok(());
    };
    apply_bounds(&webview, &bounds)
}

pub fn set_visible(app: &AppHandle, visible: bool) -> Result<(), String> {
    let Some(webview) = app.get_webview(LABEL) else {
        return Ok(());
    };
    if visible {
        webview
            .show()
            .map_err(|err| format!("无法显示 DeepSeek Web：{err}"))?;
        raise_parent_resize_overlay(&webview);
        Ok(())
    } else {
        webview.hide().map_err(|err| format!("无法隐藏 DeepSeek Web：{err}"))
    }
}

pub fn close(app: &AppHandle) -> Result<(), String> {
    if let Some(webview) = app.get_webview(LABEL) {
        let _ = webview.close();
    }
    if let Some(hub) = app.try_state::<std::sync::Arc<Hub>>() {
        if let Ok(mut guard) = hub.origin.lock() {
            *guard = None;
        }
    }
    Ok(())
}

pub fn apply_theme(app: &AppHandle, script: String) -> Result<(), String> {
    store_theme(app, Some(&script));
    let Some(webview) = app.get_webview(LABEL) else {
        return Ok(());
    };
    let _ = webview.set_background_color(Some(Color(0, 0, 0, 0)));
    webview
        .eval(&script)
        .map_err(|err| format!("无法同步 DeepSeek 外观：{err}"))
}

pub fn on_loaded(webview: &tauri::Webview) {
    if webview.label() != LABEL {
        return;
    }
    paint(webview, stored_theme(&webview.app_handle()).as_deref());
}

fn store_theme(app: &AppHandle, script: Option<&str>) {
    let Some(hub) = app.try_state::<std::sync::Arc<Hub>>() else {
        return;
    };
    let Ok(mut guard) = hub.theme_script.lock() else {
        return;
    };
    *guard = script
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
}

fn stored_theme(app: &AppHandle) -> Option<String> {
    app.try_state::<std::sync::Arc<Hub>>()
        .and_then(|hub| hub.theme_script.lock().ok().and_then(|guard| guard.clone()))
}

fn paint(webview: &tauri::Webview, script: Option<&str>) {
    let _ = webview.set_background_color(Some(Color(0, 0, 0, 0)));
    if let Some(script) = script.map(str::trim).filter(|value| !value.is_empty()) {
        let _ = webview.eval(script);
    }
}

fn apply_bounds(webview: &tauri::Webview, bounds: &Bounds) -> Result<(), String> {
    webview
        .set_position(LogicalPosition::new(bounds.x, bounds.y))
        .map_err(|err| format!("无法放置 DeepSeek Web：{err}"))?;
    webview
        .set_size(LogicalSize::new(bounds.width.max(1.0), bounds.height.max(1.0)))
        .map_err(|err| format!("无法调整 DeepSeek Web 大小：{err}"))?;
    raise_parent_resize_overlay(webview);
    Ok(())
}

fn raise_parent_resize_overlay(webview: &tauri::Webview) {
    if let Ok(hwnd) = webview.window().hwnd() {
        crate::window_chrome::raise_resize_overlay(hwnd.0 as isize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glass_init_script_appends_the_theme_iife() {
        let boot = glass_init_script(None);
        assert!(boot.trim_start().starts_with("(function"));
        assert!(boot.contains("ad-dsh-glass"));
        assert!(!boot.contains("window.__adDshGlass({bg:'#0D0D0D'})"));
        let with_theme = glass_init_script(Some("window.__adDshGlass({bg:'#0D0D0D'})"));
        assert!(with_theme.contains("ad-dsh-glass"));
        assert!(with_theme.contains("window.__adDshGlass({bg:'#0D0D0D'})"));
    }

    #[test]
    fn origin_key_strips_token_and_hash() {
        assert_eq!(
            origin_key("http://127.0.0.1:3099/?token=abc.def#ad=1"),
            "http://127.0.0.1:3099"
        );
        assert_eq!(
            origin_key("http://127.0.0.1:3099/"),
            "http://127.0.0.1:3099"
        );
        assert_ne!(
            origin_key("http://127.0.0.1:3099/?token=a"),
            origin_key("http://127.0.0.1:3100/?token=a")
        );
    }
}
