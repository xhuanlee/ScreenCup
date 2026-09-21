#![allow(clippy::too_many_lines)]

mod audio;
mod capture;
mod commands;
mod error;
mod events;
pub mod ffmpeg;
mod geometry;
mod overlay;
mod recorder;
mod settings;
mod state;
mod still;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState, ShortcutWrapper};

use crate::error::AppResult;
use crate::state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // Log to a file: a GUI app has no visible stdout, so without this
            // there is nothing to attach to a bug report.
            if let Ok(log_dir) = app.path().app_log_dir() {
                let _ = std::fs::create_dir_all(&log_dir);
                let path = log_dir.join("screencut.log");
                // Rotate the previous run out if the log grew past 1 MB.
                if let Ok(meta) = std::fs::metadata(&path) {
                    if meta.len() > 1_000_000 {
                        let _ = std::fs::rename(&path, log_dir.join("screencut.log.old"));
                    }
                }
                if let Ok(file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    let _ = env_logger::Builder::from_env(
                        env_logger::Env::default().default_filter_or("info"),
                    )
                    .format_timestamp_secs()
                    .target(env_logger::Target::Pipe(Box::new(file)))
                    .try_init();
                    log::info!("logging to {}", path.display());
                }
            }

            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(&cache_dir)?;
            log::info!("data dir: {}", data_dir.display());
            log::info!("cache dir: {}", cache_dir.display());

            app.manage(AppState::new(data_dir, cache_dir));

            // macOS quarantines any app downloaded from the internet (a DMG
            // from a browser gets `com.apple.quarantine` on every file inside
            // it). A quarantined app that is only ad-hoc signed is in a
            // degraded state: tccd reports "Failed to match existing code
            // requirement" for every screen-capture check, so the user can
            // toggle the Screen Recording switch in System Settings until
            // they are blue in the face and permission never lands. Clearing
            // the attribute restores normal TCC behaviour — verified by hand
            // on macOS 15.7: same binary, permission false while quarantined,
            // true immediately after `xattr -cr`.
            //
            // The user owns the bundle they dragged out of the DMG, so this
            // needs no extra privileges.
            #[cfg(target_os = "macos")]
            clear_quarantine_if_needed();

            log::info!(
                "screen recording permission: {}",
                scap::has_permission()
            );

            let handle = app.handle();
            apply_window_chrome(handle)?;
            register_hotkeys(handle)?;
            build_tray(handle)?;

            if let Some(win) = app.get_webview_window("main") {
                win.show()?;
                win.set_focus()?;
                // Headless self-test channel: `SCREENCUT_E2E=1` flips the main
                // window into a scripted record/stop/verify flow (see main.tsx).
                if std::env::var("SCREENCUT_E2E").is_ok() {
                    // Mode selects the scripted scenario: plain (video only),
                    // audio (system audio), mic, window, region, pause.
                    let mode = std::env::var("SCREENCUT_E2E_MODE")
                        .ok()
                        .filter(|m| !m.is_empty())
                        .unwrap_or_else(|| "plain".to_string());
                    let _ = win.eval(&format!("window.location.hash = '#e2e-{mode}'"));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::list_sources,
            commands::list_microphones,
            commands::check_permission,
            commands::request_permission,
            commands::get_settings,
            commands::save_settings,
            commands::open_region_overlay,
            commands::confirm_region,
            commands::cancel_region,
            commands::start_recording,
            commands::stop_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::get_recording_state,
            commands::get_last_result,
            commands::choose_output_dir,
            commands::reveal_file,
            commands::delete_file,
            commands::log_frontend,
            commands::probe_file,
            commands::grab_region_still,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Platform-specific chrome for the main window: macOS keeps native
/// decorations with an overlay title bar; Windows goes frameless.
fn apply_window_chrome(app: &tauri::AppHandle) -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        if let Some(win) = app.get_webview_window("main") {
            win.set_title_bar_style(tauri::TitleBarStyle::Overlay)?;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(win) = app.get_webview_window("main") {
            win.set_decorations(false)?;
        }
    }
    let _ = app;
    Ok(())
}

fn register_hotkeys(app: &tauri::AppHandle) -> AppResult<()> {
    let gs = app.global_shortcut();

    let app_handle = app.clone();
    gs.on_shortcut(
        ShortcutWrapper::try_from("CommandOrControl+Shift+R")
            .map_err(|e| error::AppError::Internal(format!("快捷键注册失败: {e}")))?,
        move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = app_handle.emit("screencut://hotkey", "toggle");
            }
        },
    )
    .map_err(|e| error::AppError::Internal(format!("快捷键注册失败: {e}")))?;

    let app_handle = app.clone();
    gs.on_shortcut(
        ShortcutWrapper::try_from("CommandOrControl+Shift+P")
            .map_err(|e| error::AppError::Internal(format!("快捷键注册失败: {e}")))?,
        move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = app_handle.emit("screencut://hotkey", "pause-toggle");
            }
        },
    )
    .map_err(|e| error::AppError::Internal(format!("快捷键注册失败: {e}")))?;

    log::info!("global shortcuts registered");
    Ok(())
}

fn build_tray(app: &tauri::AppHandle) -> AppResult<()> {
    let show = tauri::menu::MenuItemBuilder::with_id("show", "显示主窗口")
        .build(app)?;
    let toggle = tauri::menu::MenuItemBuilder::with_id("toggle", "开始 / 停止录制")
        .build(app)?;
    let quit = tauri::menu::MenuItemBuilder::with_id("quit", "退出 ScreenCut")
        .build(app)?;
    // tauri's MenuBuilder only assembles a flat menu; the item helpers live on
    // SubmenuBuilder, so wrap the items in a single submenu.
    let menu = tauri::menu::Menu::new(app)?;
    let submenu = tauri::menu::SubmenuBuilder::new(app, "ScreenCut")
        .item(&show)
        .item(&toggle)
        .separator()
        .item(&quit)
        .build()?;
    menu.append(&submenu)?;

    let mut tray = tauri::tray::TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "toggle" => {
                let _ = app.emit("screencut://hotkey", "toggle");
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

/// Strip the macOS quarantine attribute from our own bundle.
///
/// A browser-downloaded DMG marks every file it contains with
/// `com.apple.quarantine`. While that flag is present, tccd refuses to match
/// the app's code requirement for `kTCCServiceScreenCapture`, so the Screen
/// Recording toggle in System Settings has no effect on the running process.
/// Removing the flag (which the owning user may do without elevated
/// privileges) restores normal permission handling.
///
/// Returns the bundle path if the attribute was present and removed, so the
/// caller can log it and the UI can ask the user to restart.
#[cfg(target_os = "macos")]
fn clear_quarantine_if_needed() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    // Walk up from Contents/MacOS/<binary> to the .app bundle itself.
    let mut bundle = exe.clone();
    for ancestor in exe.ancestors() {
        if ancestor
            .extension()
            .is_some_and(|ext| ext == "app")
        {
            bundle = ancestor.to_path_buf();
            break;
        }
    }

    // `xattr -p` would be the direct check, but reading the attribute from
    // Rust needs an extra crate; `xattr -cr` is idempotent and cheap, so just
    // probe for the flag's presence first with the same tool.
    let has_flag = std::process::Command::new("xattr")
        .arg("-p")
        .arg("com.apple.quarantine")
        .arg(&bundle)
        .output()
        .ok()
        .is_some_and(|out| out.status.success());

    if !has_flag {
        return None;
    }

    let status = std::process::Command::new("xattr")
        .arg("-cr")
        .arg(&bundle)
        .status();

    match status {
        Ok(s) if s.success() => {
            log::info!(
                "cleared quarantine attribute from {} — restart needed for TCC to re-evaluate",
                bundle.display()
            );
            Some(bundle)
        }
        _ => {
            log::warn!(
                "failed to clear quarantine attribute from {} — screen recording permission may not be grantable until it is removed manually",
                bundle.display()
            );
            None
        }
    }
}
