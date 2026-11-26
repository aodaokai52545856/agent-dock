use tauri::{Emitter, EventTarget, WebviewWindow};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{WA_INACTIVE, WM_ACTIVATE, WM_NCDESTROY};

// WebView2 can keep its page visible and omit GotFocus when the top-level HWND
// becomes active. Observe the HWND itself, without changing keyboard focus.
pub fn attach_activation_listener(win: &WebviewWindow) {
    let handle = win.clone();
    let _ = win.run_on_main_thread(move || {
        let Ok(hwnd) = handle.hwnd() else { return };
        let (sender, mut events) = unbounded_channel::<bool>();
        let context = Box::into_raw(Box::new(sender));
        // The subclass owns the sender until WM_NCDESTROY. Its address is also
        // the subclass ID so a registration can never replace another's data.
        if unsafe {
            SetWindowSubclass(
                hwnd.0 as HWND,
                Some(activation_proc),
                context as usize,
                context as usize,
            )
        } == 0
        {
            unsafe { drop(Box::from_raw(context)) };
            return;
        }
        // Emit in message order outside the window procedure. A synchronous
        // activation may be running inside a Tauri window operation already.
        tauri::async_runtime::spawn(async move {
            while let Some(active) = events.recv().await {
                let _ = handle.emit_to(
                    EventTarget::window(handle.label()),
                    "agent-dock-window-activation",
                    active,
                );
            }
        });
    });
}

unsafe extern "system" fn activation_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    let sender = data as *mut UnboundedSender<bool>;
    match message {
        WM_ACTIVATE => {
            let active = wparam & 0xffff != WA_INACTIVE as usize && wparam >> 16 == 0;
            let _ = (*sender).send(active);
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(activation_proc), id);
            drop(Box::from_raw(sender));
        }
        _ => {}
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

pub fn attach_webview_position_sync(win: &WebviewWindow) {
    let window = win.as_ref().window();
    let handle = window.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::Moved(_)
                | tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::ScaleFactorChanged { .. }
        ) {
            notify_webview_position(&handle);
        }
    });
    notify_webview_position(&window);
}

fn notify_webview_position(window: &tauri::Window) {
    // Tauri's `unstable` multi-webview mode builds even the main view as a
    // child. Wry only forwards parent moves for non-child views, so WebView2
    // otherwise positions the first native <select> popup using stale screen
    // coordinates. Notify every child, including views added after startup.
    for webview in window.webviews() {
        let _ = webview.with_webview(|platform| unsafe {
            let _ = platform.controller().NotifyParentWindowPositionChanged();
        });
    }
}

pub fn apply(win: &WebviewWindow) {
    let Ok(hwnd) = win.hwnd() else {
        return;
    };
    let round = !win.is_maximized().unwrap_or(false);
    set_corner_preference(hwnd.0 as isize, round);
}

pub fn set_frost(win: &WebviewWindow, frost: u32) {
    let Ok(hwnd) = win.hwnd() else {
        return;
    };
    let hwnd = hwnd.0 as isize;
    set_dark_mode(hwnd);
    reset_frame(hwnd);
    set_system_backdrop(hwnd, backdrop_for_frost(frost));
}

pub fn raise_resize_overlay(hwnd: isize) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, SetWindowPos, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER,
        SWP_NOSIZE,
    };
    let parent = hwnd as HWND;
    if parent.is_null() {
        return;
    }
    let class: Vec<u16> = "TAURI_DRAG_RESIZE_BORDERS\0".encode_utf16().collect();
    let name: Vec<u16> = "TAURI_DRAG_RESIZE_WINDOW\0".encode_utf16().collect();
    let child =
        unsafe { FindWindowExW(parent, std::ptr::null_mut(), class.as_ptr(), name.as_ptr()) };
    if child.is_null() {
        return;
    }
    unsafe {
        let _ = SetWindowPos(
            child,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        );
    }
}

fn backdrop_for_frost(frost: u32) -> u32 {
    const DWMSBT_NONE: u32 = 1;
    const DWMSBT_MAINWINDOW: u32 = 2;
    const DWMSBT_TRANSIENTWINDOW: u32 = 3;
    if frost == 0 {
        DWMSBT_NONE
    } else if frost < 50 {
        DWMSBT_MAINWINDOW
    } else {
        DWMSBT_TRANSIENTWINDOW
    }
}

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

fn set_corner_preference(hwnd: isize, round: bool) {
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: u32 = 2;
    const DWMWCP_DONOTROUND: u32 = 1;
    let pref: u32 = if round {
        DWMWCP_ROUND
    } else {
        DWMWCP_DONOTROUND
    };
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&pref as *const u32).cast(),
            4,
        );
    }
}

fn set_dark_mode(hwnd: isize) {
    const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
    let dark: i32 = 1;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const i32).cast(),
            4,
        );
    }
}

fn reset_frame(hwnd: isize) {
    let margins = [0i32, 0, 0, 0];
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, margins.as_ptr());
    }
}

fn set_system_backdrop(hwnd: isize, backdrop: u32) {
    const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&backdrop as *const u32).cast(),
            4,
        );
    }
}
