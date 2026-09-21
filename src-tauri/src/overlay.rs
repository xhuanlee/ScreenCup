//! Auxiliary windows: the region-selection overlay and the floating
//! recording bar. Also collects our own window ids so scap can exclude them
//! from display capture on macOS.

use tauri::{AppHandle, Manager, Monitor, WebviewUrl, WebviewWindowBuilder};

use crate::error::{AppError, AppResult};
use crate::geometry::Rect;

pub const REGION_LABEL: &str = "screencut-region";
pub const BAR_LABEL: &str = "screencut-bar";

/// Create a transparent full-screen overlay on the monitor that matches the
/// chosen capture target, so the user can drag out a region.
pub fn create_region_overlay(app: &AppHandle, target_id: u32) -> AppResult<()> {
    close_region_overlay(app);

    let monitor = pick_monitor_for_target(app, target_id)?;
    let scale = monitor.scale_factor();
    let pos: tauri::LogicalPosition<f64> = monitor.position().to_logical(scale);
    let size: tauri::LogicalSize<f64> = monitor.size().to_logical(scale);

    let mut builder = WebviewWindowBuilder::new(
        app,
        REGION_LABEL,
        WebviewUrl::App("index.html#region".into()),
    )
    .title("")
    .inner_size(size.width, size.height)
    .position(pos.x, pos.y)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(true)
    .visible(false);

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
    }

    let win = builder.build()?;
    // Show without stealing animate-in; also bring to front.
    win.show()?;
    win.set_focus()?;
    Ok(())
}

pub fn close_region_overlay(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(REGION_LABEL) {
        let _ = win.close();
    }
}

/// Position the floating recording bar on a sensible monitor: prefer one that
/// is not being recorded (Windows has no window-exclusion API), otherwise the
/// main monitor. The bar is placed at the bottom-centre.
pub fn show_recording_bar(app: &AppHandle, recorded_target_id: Option<u32>) -> AppResult<()> {
    close_recording_bar(app);

    let monitors = app.available_monitors().unwrap_or_default();
    let recorded_size = recorded_target_id.and_then(|id| {
        crate::state::with_targets(app, |reg| {
            reg.find_info(id)
                .map(|i| (i.width as u32, i.height as u32))
        })
    });

    let primary = app.primary_monitor().ok().flatten();
    let main = monitors
        .iter()
        .find(|m| is_same_monitor(m, primary.as_ref()))
        .or_else(|| monitors.first());
    let pick = if let Some((w, h)) = recorded_size {
        monitors
            .iter()
            .find(|m| {
                let s: tauri::LogicalSize<f64> = m.size().to_logical(m.scale_factor());
                (s.width as u32, s.height as u32) != (w, h)
            })
            .or(main)
    } else {
        main
    };

    let Some(monitor) = pick else {
        // Headless / display-off environments report no monitors; the bar is a
        // nicety, so skip it rather than blocking the recording.
        log::warn!("[overlay] no monitors available; skipping the recording bar");
        return Ok(());
    };

    let scale = monitor.scale_factor();
    let size: tauri::LogicalSize<f64> = monitor.size().to_logical(scale);
    let pos: tauri::LogicalPosition<f64> = monitor.position().to_logical(scale);

    let bar_w = 300.0_f64;
    let bar_h = 68.0_f64;
    let x: f64 = pos.x + (size.width - bar_w) / 2.0;
    let y: f64 = pos.y + size.height - bar_h - 24.0;

    let mut builder = WebviewWindowBuilder::new(
        app,
        BAR_LABEL,
        WebviewUrl::App("index.html#bar".into()),
    )
    .title("ScreenCut")
    .inner_size(bar_w, bar_h)
    .position(x, y)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(true)
    .focused(false)
    .visible(false);

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true);
    }

    let win = builder.build()?;
    win.show()?;
    Ok(())
}

pub fn close_recording_bar(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(BAR_LABEL) {
        let _ = win.close();
    }
}

/// Choose the Tauri monitor that corresponds to a scap display target.
fn pick_monitor_for_target(app: &AppHandle, target_id: u32) -> AppResult<Monitor> {
    let info = crate::state::with_targets(app, |reg| reg.find_info(target_id).cloned());

    let monitors = app.available_monitors().unwrap_or_default();
    if monitors.is_empty() {
        return Err(AppError::Internal("没有可用的显示器".into()));
    }

    if let Some(info) = info {
        // Prefer an exact name + size match, then size alone.
        if let Some(m) = monitors
            .iter()
            .find(|m| m.name().is_some_and(|n| n.as_str() == info.title.as_str()))
        {
            return Ok(m.clone());
        }
        if let Some(m) = monitors.iter().find(|m| {
            let s: tauri::LogicalSize<f64> = m.size().to_logical(m.scale_factor());
            (s.width as u32, s.height as u32) == (info.width, info.height)
        }) {
            return Ok(m.clone());
        }
    }

    let primary = app.primary_monitor().ok().flatten();
    monitors
        .iter()
        .find(|m| is_same_monitor(m, primary.as_ref()))
        .or_else(|| monitors.first())
        .cloned()
        .ok_or_else(|| AppError::Internal("没有可用的显示器".into()))
}

/// Compare two monitors by name and resolution (Monitor has no PartialEq).
fn is_same_monitor(a: &Monitor, b: Option<&Monitor>) -> bool {
    let Some(b) = b else {
        return false;
    };
    a.name() == b.name()
        && a.size().width == b.size().width
        && a.size().height == b.size().height
}

/// NSWindow windowNumbers for our own windows, so scap can exclude them from
/// display capture (macOS only — Windows Graphics Capture has no equivalent,
/// the bar is positioned off the recorded monitor instead).
#[cfg(target_os = "macos")]
pub fn own_window_numbers(app: &AppHandle) -> Vec<u32> {
    use objc::{msg_send, sel, sel_impl};

    let mut out = Vec::new();
    for (_, window) in app.webview_windows() {
        let Ok(ns_window) = window.ns_window() else {
            continue;
        };
        let ns_window = ns_window as *mut objc::runtime::Object;
        if ns_window.is_null() {
            continue;
        }
        let number: i64 = unsafe { msg_send![ns_window, windowNumber] };
        if number > 0 {
            out.push(number as u32);
        }
    }
    out
}

#[cfg(not(target_os = "macos"))]
pub fn own_window_numbers(_app: &AppHandle) -> Vec<u32> {
    Vec::new()
}

/// Normalise a selection dragged inside the region overlay into a rect that is
/// valid in display-local logical coordinates.
pub fn normalise_selection(x1: f64, y1: f64, x2: f64, y2: f64) -> Rect {
    Rect::from_points(x1, y1, x2, y2)
}
