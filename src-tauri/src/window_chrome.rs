use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

static REVEALED: AtomicBool = AtomicBool::new(false);

pub fn attach(win: &WebviewWindow, on_close: impl Fn() + Send + Sync + 'static) {
    apply_window_icon(win);
    // Opaque first. Acrylic + a transparent webview before the first paint
    // is the blank gray window.
    let _ = win.set_background_color(Some(tauri::window::Color(11, 15, 19, 255)));
    apply(win);

    let handle = win.clone();
    let on_close = std::sync::Arc::new(on_close);
    win.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Resized(_)) {
            apply(&handle);
        }
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) {
            on_close();
        }
    });

    let fallback = win.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        if !REVEALED.load(Ordering::SeqCst) {
            let _ = fallback.show();
        }
    });
}

pub fn reveal(win: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        let first = !REVEALED.swap(true, Ordering::SeqCst);
        let handle = win.clone();
        let _ = win.run_on_main_thread(move || macos::reveal(&handle, first));
    }
    #[cfg(not(target_os = "macos"))]
    reveal_desktop(win);
}

#[cfg(not(target_os = "macos"))]
fn reveal_desktop(win: &WebviewWindow) {
    let first = !REVEALED.swap(true, Ordering::SeqCst);
    if first {
        apply_desktop_glass(win);
        let _ = win.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
    }
    let _ = win.show();
    if first {
        let _ = win.set_focus();
    }
}

pub fn raise_resize_overlay(hwnd: isize) {
    #[cfg(windows)]
    windows::raise_resize_overlay(hwnd);
    #[cfg(not(windows))]
    {
        let _ = hwnd;
    }
}

pub fn apply(win: &WebviewWindow) {
    #[cfg(windows)]
    windows::apply(win);
    #[cfg(target_os = "macos")]
    {
        let handle = win.clone();
        let _ = win.run_on_main_thread(move || macos::apply(&handle));
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = win;
    }
}

fn apply_window_icon(win: &WebviewWindow) {
    const ICON_PNG: &[u8] = include_bytes!("../icons/128x128.png");
    if let Ok(icon) = tauri::image::Image::from_bytes(ICON_PNG) {
        let _ = win.set_icon(icon);
    }
    refresh_shell_icons();
}

fn refresh_shell_icons() {
    #[cfg(windows)]
    {
        #[link(name = "shell32")]
        extern "system" {
            fn SHChangeNotify(event: i32, flags: u32, item1: isize, item2: isize);
        }
        const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
        const SHCNF_IDLIST: u32 = 0;
        unsafe {
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, 0, 0);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn apply_desktop_glass(win: &WebviewWindow) {
    set_frost(win, crate::state::default_ui_frost());
}

pub fn set_frost(win: &WebviewWindow, frost: u32) {
    #[cfg(windows)]
    windows::set_frost(win, frost);
    #[cfg(target_os = "macos")]
    {
        let handle = win.clone();
        let _ = win.run_on_main_thread(move || macos::set_frost(&handle, frost));
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = (win, frost);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_backdrop_mapping_stays_on_dwm_steps() {
        let src = include_str!("window_chrome/windows.rs");
        assert!(include_str!("window_chrome.rs").contains("fn reveal_desktop"));
        assert!(src.contains("DWMSBT_NONE"));
        assert!(src.contains("else if frost < 50"));
        assert!(src.contains("DWMSBT_MAINWINDOW"));
        assert!(src.contains("DWMSBT_TRANSIENTWINDOW"));
        assert!(src.contains("fn backdrop_for_frost"));
    }

    #[test]
    fn macos_native_chrome_uses_layer_radius_and_vibrancy() {
        let src = include_str!("window_chrome/macos.rs");
        let dispatcher = include_str!("window_chrome.rs");
        assert!(dispatcher.contains("run_on_main_thread"));
        assert!(src.contains("setOpaque"));
        assert!(src.contains("cornerRadius"));
        assert!(src.contains("Some(10.0)"));
        assert!(src.contains("clear_vibrancy"));
        assert!(src.contains("UnderWindowBackground"));
        assert!(src.contains("NSVisualEffectState::Active"));
        let conf = include_str!("../tauri.conf.json");
        assert!(conf.contains("macOSPrivateApi"));
    }
}
