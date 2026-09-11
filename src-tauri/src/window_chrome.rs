use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::WebviewWindow;

static REVEALED: AtomicBool = AtomicBool::new(false);

pub fn attach(win: &WebviewWindow) {
    apply_window_icon(win);
    // Opaque first. Acrylic + a transparent webview before the first paint
    // is the blank gray window.
    let _ = win.set_background_color(Some(tauri::window::Color(13, 13, 13, 255)));
    apply(win);

    let handle = win.clone();
    win.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Resized(_)) {
            apply(&handle);
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

pub fn apply(win: &WebviewWindow) {
    #[cfg(windows)]
    {
        let Ok(hwnd) = win.hwnd() else {
            return;
        };
        let round = !win.is_maximized().unwrap_or(false);
        set_corner_preference(hwnd.0 as isize, round);
    }
    #[cfg(not(windows))]
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

fn apply_desktop_glass(win: &WebviewWindow) {
    #[cfg(windows)]
    {
        let Ok(hwnd) = win.hwnd() else {
            return;
        };
        let hwnd = hwnd.0 as isize;
        set_dark_mode(hwnd);
        reset_frame(hwnd);
        set_system_backdrop(hwnd);
    }
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
        let _ = apply_vibrancy(win, NSVisualEffectMaterial::HudWindow, None, None);
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = win;
    }
}

#[cfg(windows)]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: isize,
        dw_attribute: u32,
        pv_attribute: *const core::ffi::c_void,
        cb_attribute: u32,
    ) -> i32;
    fn DwmExtendFrameIntoClientArea(hwnd: isize, margins: *const i32) -> i32;
}

#[cfg(windows)]
fn set_corner_preference(hwnd: isize, round: bool) {
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;
    const DWMWCP_DONOTROUND: u32 = 1;
    let pref: u32 = if round { DWMWCP_ROUND } else { DWMWCP_DONOTROUND };
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, (&pref as *const u32).cast(), 4);
    }
}

#[cfg(windows)]
fn set_dark_mode(hwnd: isize) {
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    let dark: i32 = 1;
    unsafe {
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, (&dark as *const i32).cast(), 4);
    }
}

#[cfg(windows)]
fn reset_frame(hwnd: isize) {
    let margins = [0i32, 0, 0, 0];
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, margins.as_ptr());
    }
}

#[cfg(windows)]
fn set_system_backdrop(hwnd: isize) {
    const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
    const DWMSBT_TRANSIENTWINDOW: u32 = 3;
    let backdrop: u32 = DWMSBT_TRANSIENTWINDOW;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&backdrop as *const u32).cast(),
            4,
        );
    }
}
