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

            // macOS quarantines any app downloaded from the internet (a DMG
            // from a browser gets `com.apple.quarantine` on the bundle). While
            // that flag is present, tccd reports "Failed to match existing
            // code requirement" for every screen-capture check, so the Screen
            // Recording toggle in System Settings has no effect. Removing the
            // flag alone is not enough: tccd captured the quarantined identity
            // when *this* process was launched, so a grant the user makes now
            // does not match and they end up authorising twice — once for the
            // quarantined copy and once for the clean one. Clearing the flag
            // and then relaunching through LaunchServices gives the new
            // process a clean identity, so a single authorization sticks. The
            // relaunch happens before any window, tray or shortcut exists, so
            // the user only sees the dock icon bounce once more.
            //
            // The user owns the bundle they dragged out of the DMG, so
            // clearing the attribute needs no extra privileges.
            #[cfg(target_os = "macos")]
            {
                let cache_dir = cache_dir.clone();
                dequarantine_and_relaunch_if_needed(&cache_dir);
            }

            app.manage(AppState::new(data_dir, cache_dir));

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

/// Strip the macOS quarantine attribute from our own bundle and, when we
/// were launched while quarantined, relaunch through LaunchServices.
///
/// A browser-downloaded DMG marks every file it contains with
/// `com.apple.quarantine`. While that flag is present, tccd refuses to match
/// the app's code requirement for `kTCCServiceScreenCapture`, so the Screen
/// Recording toggle in System Settings has no effect on the running process.
/// macOS also refuses to run a quarantined ad-hoc-signed binary directly, so
/// the first launch has to go through Gatekeeper's "Open Anyway" gate.
///
/// Clearing the attribute mid-run is not enough on its own: tccd captured the
/// quarantined identity at launch, so a grant the user makes this session
/// still does not match. The UI would then look stuck and the user would be
/// forced to authorize twice (the "works after a restart" report). Relaunching
/// through `open -n` — the same path a manual restart takes — lets tccd
/// re-register the process with the now-clean identity, so one authorization
/// is enough.
///
/// Complication: a quarantined app launched via LaunchServices is run from a
/// read-only App Translocation sandbox (`/private/var/folders/.../AppTranslocation`),
/// so `xattr -cr` on *that* copy always fails. We clear the attribute on the
/// real installed bundle instead — the translocated copy is a symlink-free
/// dead end, but the original path stays writable and is what the next launch
/// uses once the user has cleared Gatekeeper.
///
/// To avoid a relaunch loop (the flag cannot be cleared, or a policy keeps
/// re-applying it) we only relaunch when the attribute really is gone, and we
/// leave a marker file so a relaunch within a short window does not try
/// again.
#[cfg(target_os = "macos")]
fn dequarantine_and_relaunch_if_needed(cache_dir: &std::path::Path) {
    // `open -n` cannot be tested headlessly, and the e2e harness launches the
    // binary directly, so stay out of the way there.
    if std::env::var("SCREENCUT_E2E").is_ok() {
        return;
    }

    let Ok(exe) = std::env::current_exe() else {
        return;
    };
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

    // Reading the attribute from Rust needs an extra crate; `xattr -p` is the
    // same probe the shell would use and `xattr -cr` is idempotent.
    let has_flag = |path: &std::path::Path| {
        std::process::Command::new("xattr")
            .arg("-p")
            .arg("com.apple.quarantine")
            .arg(path)
            .output()
            .ok()
            .is_some_and(|out| out.status.success())
    };

    // The translocated copy always carries the flag; the interesting question
    // is whether the installed bundle does.
    let installed = if is_translocated(&bundle) {
        // The App Translocation path is `/private/var/folders/.../AppTranslocation/<UUID>/d/<bundle>`.
        // There is no reliable way back to the original from here, so fall
        // back to the conventional install location.
        let candidate = std::path::Path::new("/Applications").join(bundle.file_name().unwrap_or_default());
        if has_flag(&candidate) {
            candidate
        } else {
            // The real bundle is clean already; nothing to do.
            return;
        }
    } else {
        if !has_flag(&bundle) {
            return;
        }
        bundle.clone()
    };

    let marker = cache_dir.join("quarantine-relaunch.marker");
    if let Ok(meta) = std::fs::metadata(&marker) {
        if let Ok(modified) = modified_time(&meta) {
            // A relaunch in the last minute means clearing the attribute did
            // not stick (a management profile re-applying it, for example) —
            // bail out instead of spinning.
            if modified.elapsed().unwrap_or(std::time::Duration::MAX)
                < std::time::Duration::from_secs(60)
            {
                log::warn!(
                    "quarantine attribute re-appeared right after a relaunch; \
                     leaving it in place rather than looping"
                );
                return;
            }
        }
    }

    let cleared = std::process::Command::new("xattr")
        .arg("-cr")
        .arg(&installed)
        .status()
        .ok()
        .is_some_and(|s| s.success());

    if !cleared || has_flag(&installed) {
        log::warn!(
            "failed to clear quarantine attribute from {} — screen recording \
             permission may not be grantable until it is removed manually \
             (`xattr -cr {}`)",
            installed.display(),
            installed.display()
        );
        return;
    }

    log::info!(
        "cleared quarantine attribute from {}",
        installed.display()
    );
    let _ = std::fs::write(&marker, b"");

    // Permission may already be fine (the user granted it to a clean copy
    // earlier and the attribute came back with a re-download); there is
    // nothing to gain from a relaunch then, so skip it.
    if scap::has_permission() {
        log::info!("screen recording permission already granted; skipping relaunch");
        return;
    }

    log::info!("relaunching through LaunchServices so tccd re-registers us without quarantine");
    let open = std::process::Command::new("open")
        .arg("-n")
        .arg(&installed)
        .spawn();
    match open {
        Ok(child) => {
            log::info!("relaunched as pid {}; exiting this instance", child.id());
            // tauri's own restart path fork/execs, which would inherit this
            // process's quarantine label — exiting here lets the
            // LaunchServices-spawned copy take over with a clean identity.
            std::process::exit(0);
        }
        Err(e) => {
            log::warn!(
                "could not relaunch via `open`: {e}; continuing in this process"
            );
        }
    }
}

/// True when the bundle path is the read-only App Translocation sandbox macOS
/// runs quarantined apps from.
#[cfg(target_os = "macos")]
fn is_translocated(bundle: &std::path::Path) -> bool {
    bundle
        .to_string_lossy()
        .contains("/private/var/folders/")
        && bundle
            .to_string_lossy()
            .contains("AppTranslocation")
}

#[cfg(target_os = "macos")]
fn modified_time(meta: &std::fs::Metadata) -> std::io::Result<std::time::SystemTime> {
    meta.modified()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[cfg(target_os = "macos")]
    #[test]
    fn detects_app_translocation_paths() {
        let translocated = PathBuf::from(
            "/private/var/folders/33/vwc7vh2s0j92vx3g28yfw2y40000gp/T/\
             AppTranslocation/645946F6-DE85-4F69-A99E-6B29872A9B09/d/ScreenCut.app",
        );
        assert!(is_translocated(&translocated));

        // The real install location must not be mistaken for the sandbox.
        let installed = PathBuf::from("/Applications/ScreenCut.app");
        assert!(!is_translocated(&installed));

        // A dev build running out of target/.
        let dev = PathBuf::from("/Users/dev/ScreenCut/src-tauri/target/release/screencut");
        assert!(!is_translocated(&dev));

        // /private/var/folders without AppTranslocation is a normal temp path.
        let temp = PathBuf::from("/private/var/folders/xx/T/ScreenCut.app");
        assert!(!is_translocated(&temp));
    }

    #[test]
    fn walk_up_from_binary_finds_bundle() {
        // Mirror the ancestor walk used by the quarantine self-heal: starting
        // at Contents/MacOS/<binary>, the first `.app` ancestor is the bundle.
        let exe = PathBuf::from("/Applications/ScreenCut.app/Contents/MacOS/screencut");
        let mut bundle = exe.clone();
        for ancestor in exe.ancestors() {
            if ancestor.extension().is_some_and(|ext| ext == "app") {
                bundle = ancestor.to_path_buf();
                break;
            }
        }
        assert_eq!(bundle, PathBuf::from("/Applications/ScreenCut.app"));
    }
}
