use objc2::MainThreadMarker;
use objc2_app_kit::{NSColor, NSWindow};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tauri::WebviewWindow;
use window_vibrancy::{
    apply_vibrancy, clear_vibrancy, NSVisualEffectMaterial, NSVisualEffectState,
};

/// Matches `--ad-radius-window`. NSVisualEffectView + CALayer cornerRadius use this.
const WINDOW_CORNER_RADIUS: f64 = 10.0;

static LAST_FROST: AtomicU32 = AtomicU32::new(0);
static LAST_ROUNDED: AtomicBool = AtomicBool::new(true);

pub fn apply(win: &WebviewWindow) {
    let radius = corner_radius(win);
    apply_native(win, radius);
    let rounded = radius > 0.0;
    if LAST_ROUNDED.swap(rounded, Ordering::SeqCst) != rounded {
        apply_frost_material(win, LAST_FROST.load(Ordering::SeqCst), radius);
    }
}

pub fn reveal(win: &WebviewWindow, first: bool) {
    if first {
        set_frost(win, crate::state::default_ui_frost());
        let _ = win.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
    }
    let _ = win.show();
    if first {
        let _ = win.set_focus();
    }
}

pub fn set_frost(win: &WebviewWindow, frost: u32) {
    LAST_FROST.store(frost, Ordering::SeqCst);
    let radius = corner_radius(win);
    LAST_ROUNDED.store(radius > 0.0, Ordering::SeqCst);
    apply_native(win, radius);
    apply_frost_material(win, frost, radius);
}

fn corner_radius(win: &WebviewWindow) -> f64 {
    if win.is_maximized().unwrap_or(false) {
        0.0
    } else {
        WINDOW_CORNER_RADIUS
    }
}

fn apply_native(win: &WebviewWindow, radius: f64) {
    let Some(_mtm) = MainThreadMarker::new() else {
        return;
    };
    let Ok(ptr) = win.ns_window() else {
        return;
    };
    if ptr.is_null() {
        return;
    }
    let ns_window = unsafe { &*ptr.cast::<NSWindow>() };
    ns_window.setOpaque(false);
    ns_window.setBackgroundColor(Some(&NSColor::clearColor()));
    ns_window.setHasShadow(true);
    let Some(view) = ns_window.contentView() else {
        return;
    };
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        layer.setCornerRadius(radius);
        layer.setMasksToBounds(true);
    }
}

fn apply_frost_material(win: &WebviewWindow, frost: u32, radius: f64) {
    let _ = clear_vibrancy(win);
    if frost == 0 {
        return;
    }
    let material = if frost < 50 {
        NSVisualEffectMaterial::UnderWindowBackground
    } else {
        NSVisualEffectMaterial::HudWindow
    };
    let vibrancy_radius = if radius > 0.0 { Some(10.0) } else { Some(0.0) };
    let _ = apply_vibrancy(
        win,
        material,
        Some(NSVisualEffectState::Active),
        vibrancy_radius,
    );
}
