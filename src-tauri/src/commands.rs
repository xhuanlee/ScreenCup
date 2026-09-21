//! Tauri commands exposed to the frontend.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::time::timeout;

use crate::capture::{CaptureTargetInfo, SourceSpec, TargetRegistry};
use crate::error::{AppError, AppResult};
use crate::events::{AppEvent, StatePayload};
use crate::recorder::{RecordingConfig, RecordingResult};
use crate::settings::{Settings, SourceKind};
use crate::state::AppState;

const STOP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub ffmpeg_path: Option<String>,
    pub has_ffmpeg: bool,
    pub encoder: Option<String>,
    pub encoder_hardware: bool,
    pub permission_granted: bool,
    pub system_audio_supported: bool,
    pub default_output_dir: String,
    /// Where the rolling log file lives (shown in the UI so users can attach
    /// it to a bug report).
    pub log_path: Option<String>,
    /// Running from a DMG / translocated location: macOS will never grant it
    /// screen-recording permission until it is installed properly.
    pub needs_install: bool,
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> AppResult<AppInfo> {
    log::info!("[cmd] get_app_info");
    let ffmpeg = crate::ffmpeg::find_ffmpeg();
    log::info!("[cmd] get_app_info: ffmpeg={:?}", ffmpeg.as_deref());
    let (encoder, hw) = match &ffmpeg {
        Some(p) => {
            let list = crate::ffmpeg::list_encoders(p).unwrap_or_default();
            log::info!("[cmd] get_app_info: {} encoders", list.len());
            let e = crate::ffmpeg::pick_encoder(&list);
            (Some(e.name().to_string()), e.is_hardware())
        }
        None => (None, false),
    };

    log::info!("[cmd] get_app_info: resolving default output dir");
    let out_dir = default_output_dir(&app);
    log::info!("[cmd] get_app_info: out dir = {}", out_dir.display());

    let log_path = app
        .path()
        .app_log_dir()
        .ok()
        .map(|d| d.join("screencut.log").to_string_lossy().into_owned());

    let info = AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        ffmpeg_path: ffmpeg.as_ref().map(|p| p.to_string_lossy().into_owned()),
        has_ffmpeg: ffmpeg.is_some(),
        encoder,
        encoder_hardware: hw,
        permission_granted: scap::has_permission(),
        system_audio_supported: crate::capture::system_audio_supported(),
        default_output_dir: out_dir.to_string_lossy().into_owned(),
        log_path,
        // macOS quarantines apps launched straight from a mounted DMG by
        // running them from a random /private/var/folders path (or directly
        // from /Volumes). TCC never grants screen recording to those copies,
        // so the only fix is to install the app properly.
        needs_install: current_exe_is_translocated(),
    };
    log::info!(
        "[cmd] get_app_info: done (permission={}, needs_install={})",
        info.permission_granted, info.needs_install
    );
    Ok(info)
}

fn current_exe_is_translocated() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let p = exe.to_string_lossy();
    p.contains("/Volumes/") || p.contains("/private/var/folders/")
}

/// Enumerate capture targets (displays + windows) and refresh the registry.
#[tauri::command]
pub fn list_sources(app: AppHandle, state: State<'_, AppState>) -> AppResult<Vec<CaptureTargetInfo>> {
    let mut registry = TargetRegistry::refresh()?;
    log::info!("[cmd] list_sources: {} targets", registry.list().len());
    registry.fill_display_geometry(&app.available_monitors().unwrap_or_default());
    let list = registry.list();
    state.replace_targets(registry);
    let _ = app.emit(AppEvent::State.as_str(), state_payload(&app, "idle"));
    Ok(list)
}

#[tauri::command]
pub fn list_microphones() -> Vec<String> {
    log::info!("[cmd] list_microphones");
    crate::audio::list_microphone_names()
}

/// Permission check. `CGPreflightScreenCaptureAccess` can keep reporting
/// "not granted" for a translocated or renamed copy of the app even after the
/// user has granted permission, so when preflight fails we fall back to
/// actually starting a stream and looking for a frame — that is the only
/// authoritative answer.
///
/// The probe drives a futures executor that dispatches onto the main queue,
/// so it can never run on the main thread (the main run loop must be free to
/// service it). Sync commands execute on the main thread, hence the dedicated
/// worker thread here. `catch_unwind` keeps any panic in the capture stack
/// from taking the whole app down.
#[tauri::command]
pub async fn check_permission() -> bool {
    if scap::has_permission() {
        log::info!("[cmd] check_permission -> true (preflight)");
        return true;
    }
    log::info!("[cmd] check_permission: preflight said no, probing real stream");
    let handle = std::thread::spawn(|| {
        std::panic::catch_unwind(scap::probe_capture).unwrap_or(false)
    });
    match handle.join() {
        Ok(probed) => {
            log::info!("[cmd] check_permission -> {probed} (probed)");
            probed
        }
        Err(_) => {
            log::warn!("[cmd] check_permission: probe thread failed");
            false
        }
    }
}

#[tauri::command]
pub fn request_permission() -> bool {
    if scap::has_permission() {
        return true;
    }
    scap::request_permission()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    log::info!("[cmd] get_settings");
    state.settings()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> AppResult<()> {
    settings.save(&state.data_dir)?;
    state.set_settings(settings);
    let _ = app.emit(AppEvent::State.as_str(), state_payload(&app, "idle"));
    Ok(())
}

/// Grab a single frame of the target display for the region overlay's
/// magnifier loupe. Runs on a worker thread because building a capturer on
/// macOS drives a futures executor that dispatches onto the main queue.
#[tauri::command]
pub async fn grab_region_still(
    app: AppHandle,
    state: State<'_, AppState>,
    target_id: Option<u32>,
) -> AppResult<crate::still::StillFrame> {
    let target = crate::still::target_for_id(target_id)?;
    let cache_dir = state.cache_dir.clone();
    let handle = std::thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::still::grab_still(&cache_dir, &target)
        }))
        .map_err(|_| AppError::Internal("画面捕获意外终止".into()))?
    });
    let still = handle
        .join()
        .map_err(|_| AppError::Internal("画面捕获线程已退出".into()))??;
    // The overlay reads the PNG through the asset protocol.
    let _ = app.emit("screencut://still-ready", &still);
    Ok(still)
}

/// Open the transparent region-selection overlay over the chosen display.
#[tauri::command]
pub fn open_region_overlay(app: AppHandle, target_id: u32) -> AppResult<()> {
    crate::overlay::create_region_overlay(&app, target_id)
}

/// Called from the region overlay when the user confirms a selection.
#[tauri::command]
pub fn confirm_region(app: AppHandle, state: State<'_, AppState>, rect: crate::geometry::Rect) -> AppResult<()> {
    if !rect.is_valid() {
        return Err(AppError::RegionNotSelected);
    }
    crate::overlay::close_region_overlay(&app);

    let mut settings = state.settings();
    settings.region = Some(rect);
    settings.save(&state.data_dir)?;
    state.set_settings(settings);
    // The overlay is a separate webview with its own store; the main window
    // needs this event to drop the "picking" state and enable recording.
    let _ = app.emit(AppEvent::RegionConfirmed.as_str(), Some(&rect));
    Ok(())
}

#[tauri::command]
pub fn cancel_region(app: AppHandle) -> AppResult<()> {
    crate::overlay::close_region_overlay(&app);
    // Let the main window drop its "picking" state too.
    let _ = app.emit(AppEvent::RegionConfirmed.as_str(), None::<&crate::geometry::Rect>);
    Ok(())
}

#[tauri::command]
pub async fn start_recording(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    {
        let session = state.session();
        if let Some(s) = session.as_ref() {
            if s.is_running() {
                return Err(AppError::AlreadyRecording);
            }
        }
    }

    let settings = state.settings();

    // Make sure the target registry is populated before resolving anything.
    ensure_targets(&app, &state)?;
    let spec = build_source_spec(&app, &settings)?;

    let resolved = crate::state::with_targets(&app, |reg| reg.resolve(&spec))?;
    let summary = settings.recording_summary();

    let output_dir = match &settings.output_dir {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => default_output_dir(&app),
    };
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| AppError::Internal(format!("无法创建输出目录: {e}")))?;

    let output_path = unique_output_path(&output_dir);
    let temp_dir = state.cache_dir.join("recording");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| AppError::Internal(format!("无法创建临时目录: {e}")))?;

    let excluded_targets = crate::state::with_targets(&app, |reg| {
        reg.excluded_window_targets(&crate::overlay::own_window_numbers(&app))
    });

    let config = RecordingConfig {
        resolved,
        summary,
        mic_device: settings.mic_device.clone(),
        excluded_targets,
        output_path,
        temp_dir,
    };

    // Create the bar before spawning the supervisor: if window creation
    // fails, no recording thread is left running.
    crate::overlay::show_recording_bar(&app, config_target_id(&settings))?;

    let session = crate::recorder::start(app.clone(), config)?;

    if settings.hide_main_while_recording {
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.hide();
        }
    }

    *state.session() = Some(session);
    let _ = app.emit(AppEvent::State.as_str(), state_payload(&app, "recording"));
    Ok(())
}

#[tauri::command]
pub async fn stop_recording(app: AppHandle, state: State<'_, AppState>) -> AppResult<RecordingResult> {
    let session_opt = state.session().take();
    let Some(mut session) = session_opt else {
        return Err(AppError::NotRecording);
    };
    session.request_stop();
    let rx = session.take_result_rx();

    let result = if let Some(rx) = rx {
        // rx is Receiver<AppResult<RecordingResult>>, so the layers are:
        // timeout -> recv result -> supervisor result.
        match timeout(STOP_TIMEOUT, rx).await {
            Ok(Ok(Ok(res))) => res,
            Ok(Ok(Err(e))) => {
                log::error!("[cmd] recording failed: {e}");
                return Err(e);
            }
            Ok(Err(_)) => {
                log::error!("[cmd] supervisor dropped without a result");
                return Err(AppError::Internal("录制线程已退出".into()));
            }
            Err(_) => {
                log::error!("[cmd] recording did not finish within {STOP_TIMEOUT:?}");
                return Err(AppError::Internal("录制完成超时".into()));
            }
        }
    } else {
        return Err(AppError::Internal("缺少录制结果通道".into()));
    };

    if let Some(handle) = session.take_supervisor() {
        // The supervisor has already exited if the result arrived.
        if !handle.is_finished() {
            let _ = handle.join();
        }
    }
    session.detach();

    crate::overlay::close_recording_bar(&app);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }

    state.set_last_result(result.clone());
    let _ = app.emit(AppEvent::State.as_str(), state_payload(&app, "idle"));
    Ok(result)
}

#[tauri::command]
pub fn pause_recording(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let session = state.session();
    let Some(session) = session.as_ref() else {
        return Err(AppError::NotRecording);
    };
    session.pause()?;
    let _ = app.emit(AppEvent::State.as_str(), state_payload(&app, "paused"));
    Ok(())
}

#[tauri::command]
pub fn resume_recording(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let session = state.session();
    let Some(session) = session.as_ref() else {
        return Err(AppError::NotRecording);
    };
    session.resume()?;
    let _ = app.emit(AppEvent::State.as_str(), state_payload(&app, "recording"));
    Ok(())
}

#[tauri::command]
pub fn get_recording_state(state: State<'_, AppState>) -> String {
    log::info!("[cmd] get_recording_state");
    let session = state.session();
    match session.as_ref() {
        Some(s) => serde_json::to_string(&s.state()).unwrap_or_else(|_| "idle".into()),
        None => "idle".into(),
    }
}

#[tauri::command]
pub fn get_last_result(state: State<'_, AppState>) -> Option<RecordingResult> {
    log::info!("[cmd] get_last_result");
    state.last_result()
}

#[tauri::command]
pub async fn choose_output_dir(app: AppHandle) -> AppResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;

    let folder = app
        .dialog()
        .file()
        .set_title("选择录制保存位置")
        .blocking_pick_folder();
    Ok(folder.map(|p| p.to_string()))
}

#[tauri::command]
pub fn reveal_file(path: String) -> AppResult<()> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(AppError::Internal(format!("文件不存在: {path}")));
    }
    reveal(&p)
}

#[tauri::command]
pub fn delete_file(path: String) -> AppResult<()> {
    let p = PathBuf::from(&path);
    if p.exists() {
        std::fs::remove_file(&p)
            .map_err(|e| AppError::Internal(format!("删除失败: {e}")))?;
    }
    Ok(())
}

/// Receive a diagnostic line from the frontend (boot beacon / errors).
#[tauri::command]
pub fn log_frontend(message: String) {
    log::info!("[frontend] {message}");
}

/// Probe a recording with ffprobe (used by the headless self-test).
#[tauri::command]
pub fn probe_file(path: String) -> AppResult<String> {
    let meta = crate::ffmpeg::probe_video(std::path::Path::new(&path))?;
    Ok(format!(
        "{}x{} {} {:.1}s {} frames",
        meta.width,
        meta.height,
        meta.codec_name.as_deref().unwrap_or("?"),
        meta.duration,
        meta.nb_frames
    ))
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn state_payload(_app: &AppHandle, state: &str) -> StatePayload {
    // Elapsed time is streamed continuously via the `screencut://elapsed`
    // event; state payloads only carry the transition.
    StatePayload {
        state: state.to_string(),
        elapsed_ms: 0,
    }
}

fn default_output_dir(app: &AppHandle) -> PathBuf {
    let base = app
        .path()
        .video_dir()
        .ok()
        .or_else(|| app.path().home_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("ScreenCut")
}

fn config_target_id(settings: &Settings) -> Option<u32> {
    match settings.kind {
        SourceKind::Window => settings.target_id,
        SourceKind::Display | SourceKind::Region => settings.target_id,
    }
}

/// Make sure the target registry is populated.
fn ensure_targets(app: &AppHandle, state: &AppState) -> AppResult<()> {
    if crate::state::with_targets(app, |reg| reg.list().is_empty()) {
        let mut registry = TargetRegistry::refresh()?;
        registry.fill_display_geometry(&app.available_monitors().unwrap_or_default());
        if registry.list().is_empty() {
            return Err(AppError::SourceNotFound("无可用录制源".into()));
        }
        state.replace_targets(registry);
    }
    Ok(())
}

/// Turn the user's settings into a concrete source spec, resolving
/// "no target chosen" to the main display.
fn build_source_spec(app: &AppHandle, settings: &Settings) -> AppResult<SourceSpec> {
    let target_id = match settings.target_id {
        Some(id) => id,
        None => crate::state::with_targets(app, |reg| {
            reg.list()
                .into_iter()
                .find(|i| i.is_primary && i.kind == "display")
                .map(|i| i.id)
        })
        .ok_or_else(|| AppError::SourceNotFound("无显示器".into()))?,
    };

    Ok(SourceSpec {
        kind: settings.kind,
        target_id,
        region: settings.region,
    })
}

fn unique_output_path(dir: &PathBuf) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y-%m-%d %H-%M-%S").to_string();
    let mut candidate = dir.join(format!("ScreenCut {stamp}.mp4"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("ScreenCut {stamp}-{n}.mp4"));
        n += 1;
    }
    candidate
}

#[cfg(target_os = "macos")]
fn reveal(path: &std::path::Path) -> AppResult<()> {
    std::process::Command::new("open")
        .args(["-R", &path.to_string_lossy()])
        .status()
        .map_err(|e| AppError::Internal(format!("无法打开: {e}")))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn reveal(path: &std::path::Path) -> AppResult<()> {
    std::process::Command::new("explorer.exe")
        .args(["/select,", &path.to_string_lossy()])
        .status()
        .map_err(|e| AppError::Internal(format!("无法打开: {e}")))?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal(path: &std::path::Path) -> AppResult<()> {
    Err(AppError::Internal("不支持的平台".into()))
}
