use tauri::WebviewWindow;

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
