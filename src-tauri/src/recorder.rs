//! The recording session: orchestrates screen capture, audio capture and the
//! ffmpeg encoder pipeline.
//!
//! Thread layout (all spawned by one supervisor thread):
//! ```text
//! capture thread  : scap frames -> latest-frame buffer
//!                 : (macOS) scap Frame::Audio -> system-audio raw file
//! ticker thread   : latest frame -> ffmpeg stdin (raw BGR0 @ fps) -> temp mp4
//! supervisor      : cpal audio recorders -> raw pcm files
//!                 : waits for stop, then muxes video + audio into the output
//! ```

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use parking_lot::Mutex as PMutex;
use serde::Serialize;
use tauri::Emitter;
use tokio::sync::oneshot;

use crate::audio::AudioRecorder;
use crate::capture::ResolvedSource;
use crate::error::{AppError, AppResult};
use crate::events::AppEvent;
use crate::ffmpeg::{self, mux_args, spawn_with_stdin, video_record_args, RawAudioSource};
use crate::settings::RecordingSummary;

/// Everything needed to start a recording.
pub struct RecordingConfig {
    pub resolved: ResolvedSource,
    pub summary: RecordingSummary,
    pub mic_device: Option<String>,
    pub excluded_targets: Vec<scap::Target>,
    pub output_path: PathBuf,
    pub temp_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingResult {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub codec: Option<String>,
    pub frames: i64,
    pub audio: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Idle,
    Recording,
    Paused,
    Stopping,
}

/// Handle held by the app state while a recording is running.
pub struct RecordingSession {
    stop_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    state_flag: Arc<PMutex<SessionState>>,
    supervisor: Option<JoinHandle<()>>,
    result_rx: Option<oneshot::Receiver<AppResult<RecordingResult>>>,
}

impl RecordingSession {
    pub fn state(&self) -> SessionState {
        *self.state_flag.lock()
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state(), SessionState::Recording | SessionState::Paused)
    }

    pub fn pause(&self) -> AppResult<()> {
        if self.state() != SessionState::Recording {
            return Err(AppError::NotRecording);
        }
        self.pause_flag.store(true, Ordering::SeqCst);
        *self.state_flag.lock() = SessionState::Paused;
        Ok(())
    }

    pub fn resume(&self) -> AppResult<()> {
        if self.state() != SessionState::Paused {
            return Err(AppError::NotRecording);
        }
        self.pause_flag.store(false, Ordering::SeqCst);
        *self.state_flag.lock() = SessionState::Recording;
        Ok(())
    }

    /// Signal the supervisor to stop and finalise the recording.
    pub fn request_stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    /// Take the completion receiver (used by the `stop_recording` command).
    pub fn take_result_rx(&mut self) -> Option<oneshot::Receiver<AppResult<RecordingResult>>> {
        self.result_rx.take()
    }

    pub fn supervisor_mut(&mut self) -> Option<&mut JoinHandle<()>> {
        self.supervisor.as_mut()
    }

    /// Take the supervisor handle so it can be joined by value.
    pub fn take_supervisor(&mut self) -> Option<JoinHandle<()>> {
        self.supervisor.take()
    }

    /// Forget the supervisor handle (after it has been joined).
    pub fn detach(&mut self) {
        self.supervisor = None;
    }
}

/// The newest decoded frame, shared between the capture thread and the ticker.
struct FrameBuffer {
    inner: Mutex<Option<Arc<FrameBlob>>>,
}

struct FrameBlob {
    width: i32,
    height: i32,
    data: Vec<u8>,
}

impl FrameBuffer {
    fn new() -> Self {
        FrameBuffer { inner: Mutex::new(None) }
    }

    fn store(&self, blob: Arc<FrameBlob>) {
        *self.inner.lock().unwrap() = Some(blob);
    }

    fn snapshot(&self) -> Option<Arc<FrameBlob>> {
        self.inner.lock().unwrap().clone()
    }
}

/// Nearest-neighbour resample of a tightly-packed BGR24 (3 bytes/pixel)
/// buffer to a different size. Only used when the capture stream disagrees
/// with the encoder's declared input size — dropping every frame there would
/// produce a pure-black video.
fn rescale_bgr24(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    if (sw, sh) == (dw, dh) || sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return src.to_vec();
    }
    let mut out = vec![0u8; (dw as usize) * (dh as usize) * 3];
    let xstep = sw as f64 / dw as f64;
    let ystep = sh as f64 / dh as f64;
    for y in 0..dh {
        let sy = (y as f64 * ystep).floor() as usize;
        let row = sy * sw as usize * 3;
        let out_row = y as usize * dw as usize * 3;
        for x in 0..dw {
            let sx = (x as f64 * xstep).floor() as usize;
            let p = row + sx * 3;
            let q = out_row + x as usize * 3;
            out[q..q + 3].copy_from_slice(&src[p..p + 3]);
        }
    }
    out
}

/// Boot a recording session. Returns immediately; the supervisor thread does
/// the work and reports the outcome via the oneshot + app events.
pub fn start(app: tauri::AppHandle, config: RecordingConfig) -> AppResult<RecordingSession> {
    if !scap::is_supported() || !scap::has_permission() {
        return Err(AppError::ScreenPermissionDenied);
    }

    let stop_flag = Arc::new(AtomicBool::new(false));
    let pause_flag = Arc::new(AtomicBool::new(false));
    let state_flag = Arc::new(PMutex::new(SessionState::Recording));
    let (tx, rx) = oneshot::channel();

    let mut session = RecordingSession {
        stop_flag: stop_flag.clone(),
        pause_flag: pause_flag.clone(),
        state_flag: state_flag.clone(),
        supervisor: None,
        result_rx: Some(rx),
    };

    let supervisor = std::thread::Builder::new()
        .name("screencut-supervisor".into())
        .spawn(move || {
            // scap can panic deep inside SCStream construction (e.g. when the
            // display is not drawable); catch it so stop_recording gets a
            // clean error instead of hanging on a dropped channel.
            let outcome =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_supervisor(app, config, stop_flag, pause_flag, state_flag)
                }))
                .map_err(|_| AppError::Internal("屏幕捕获意外终止（SCStream 创建失败）".into()))
                .and_then(|r| r);
            if let Err(ref e) = outcome {
                log::error!("[rec] session failed: {e}");
            }
            let _ = tx.send(outcome);
        })
        .map_err(|e| AppError::Internal(format!("无法启动录制线程: {e}")))?;

    session.supervisor = Some(supervisor);
    Ok(session)
}

fn run_supervisor(
    app: tauri::AppHandle,
    config: RecordingConfig,
    stop_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
    state_flag: Arc<PMutex<SessionState>>,
) -> AppResult<RecordingResult> {
    let started = Instant::now();
    let fps = config.summary.fps;
    let mut warnings = Vec::new();

    // ---- build the capturer -------------------------------------------------
    let options = config.resolved.to_options(
        fps,
        config.summary.quality,
        config.summary.show_cursor,
        config.summary.capture_system_audio,
        config.excluded_targets.clone(),
    );

    // SCStream creation can fail transiently (leftover state after a
    // force-kill, the permission flip just having taken effect, the display
    // waking up), so retry a couple of times before giving up.
    let mut capturer = None;
    for attempt in 1..=3 {
        match scap::capturer::Capturer::build(options.clone()) {
            Ok(c) => { capturer = Some(c); break; }
            Err(e) => {
                log::warn!("[rec] capturer build attempt {attempt}/3 failed: {e:?}");
                if attempt < 3 { std::thread::sleep(Duration::from_millis(600 * attempt as u64)); }
            }
        }
    }
    let mut capturer = capturer.ok_or(AppError::ScreenPermissionDenied)?;
    let [vw, vh] = capturer.get_output_frame_size();
    if vw == 0 || vh == 0 {
        return Err(AppError::Internal("无效的捕获尺寸".into()));
    }
    log::info!("[rec] capturer built, output {vw}x{vh} @ {fps}fps");

    // ---- ffmpeg video encoder ----------------------------------------------
    let ffmpeg = ffmpeg::find_ffmpeg().ok_or(AppError::FfmpegNotFound)?;
    let encoders = ffmpeg::list_encoders(&ffmpeg).unwrap_or_default();
    let encoder = ffmpeg::pick_encoder(&encoders);
    log::info!(
        "[rec] video encoder: {} (hardware={})",
        encoder.name(),
        encoder.is_hardware()
    );

    let video_tmp = config.temp_dir.join("screencut-video.mp4");
    let video_args = video_record_args(encoder, vw, vh, fps, &video_tmp);
    let mut ffmpeg_child = spawn_with_stdin(&ffmpeg, &video_args)?;
    let ffmpeg_stdin = ffmpeg_child.stdin.take().ok_or(AppError::FfmpegFailed(1))?;

    // ---- macOS system audio pipe (shared with the capture thread) ----------
    #[cfg(target_os = "macos")]
    let mut scap_audio: Option<ScapAudioPipe> = if config.summary.capture_system_audio {
        match ScapAudioPipe::start(&config.temp_dir) {
            Ok(p) => Some(p),
            Err(e) => {
                warnings.push(format!("系统音频初始化失败: {e}"));
                None
            }
        }
    } else {
        None
    };

    // ---- capture thread -----------------------------------------------------
    let frame_buffer = Arc::new(FrameBuffer::new());
    let capture_stop = stop_flag.clone();
    let capture_pause = pause_flag.clone();

    let capture_handle: JoinHandle<()> = {
        let buffer = frame_buffer.clone();
        // The capture thread only needs the pipe's sender; the pipe itself
        // stays owned by the supervisor so it can flush it on shutdown.
        #[cfg(target_os = "macos")]
        let audio_tx: Option<mpsc::SyncSender<Vec<u8>>> =
            scap_audio.as_ref().and_then(|p| p.sender());
        std::thread::Builder::new()
            .name("screencut-capture".into())
            .spawn(move || {
                capturer.start_capture();
                loop {
                    if capture_stop.load(Ordering::SeqCst) {
                        capturer.stop_capture();
                        break;
                    }
                    match capturer.get_next_frame() {
                        Ok(scap::frame::Frame::Video(scap::frame::VideoFrame::BGR0(f))) => {
                            buffer.store(Arc::new(FrameBlob {
                                width: f.width,
                                height: f.height,
                                data: f.data,
                            }));
                        }
                        Ok(scap::frame::Frame::Audio(a)) => {
                            #[cfg(target_os = "macos")]
                            if !capture_pause.load(Ordering::SeqCst) {
                                if let Some(tx) = &audio_tx {
                                    let _ = tx.send(a.raw_data().to_vec());
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("[rec] capture stream ended: {e:?}");
                            break;
                        }
                    }
                }
                log::info!("[rec] capture thread done");
            })
            .map_err(|e| AppError::Internal(format!("无法启动捕获线程: {e}")))?
    };

    // ---- ticker thread: pump frames into ffmpeg at a constant fps ----------
    let ticker_stop = stop_flag.clone();
    let ticker_pause = pause_flag.clone();
    let ticker_buffer = frame_buffer.clone();
    let ticker_handle: JoinHandle<()> = {
        std::thread::Builder::new()
            .name("screencut-ticker".into())
            .spawn(move || {
                let period = Duration::from_secs_f64(1.0 / fps as f64);
                let mut written = 0u64;
                let mut rescaled = 0u64;
                let mut warned_size = false;
                let mut stdin = ffmpeg_stdin;
                while !ticker_stop.load(Ordering::SeqCst) {
                    let frame_start = Instant::now();
                    if !ticker_pause.load(Ordering::SeqCst) {
                        if let Some(blob) = ticker_buffer.snapshot() {
                            let (bw, bh) = (blob.width as u32, blob.height as u32);
                            let data = if bw == vw && bh == vh {
                                blob.data.as_slice()
                            } else {
                                // The stream delivered a size other than the
                                // encoder's declared input (window resized
                                // mid-recording, or the source geometry
                                // disagrees with the estimate). Dropping it
                                // would yield a pure-black file, so resample
                                // to the encoder size instead.
                                if !warned_size {
                                    warned_size = true;
                                    log::warn!(
                                        "[rec] frame size {bw}x{bh} != encoder {vw}x{vh}; resampling"
                                    );
                                }
                                rescaled += 1;
                                &rescale_bgr24(&blob.data, bw, bh, vw, vh)
                            };
                            if let Err(e) = stdin.write_all(data) {
                                log::error!("[rec] ffmpeg stdin write failed: {e}");
                                break;
                            }
                            written += 1;
                        }
                    }
                    let elapsed = frame_start.elapsed();
                    if period > elapsed {
                        std::thread::sleep(period - elapsed);
                    }
                }
                log::info!("[rec] ticker done, wrote {written} frames ({rescaled} resampled)");
            })
            .map_err(|e| AppError::Internal(format!("无法启动帧推送线程: {e}")))?
    };

    // ---- microphones (cpal) -------------------------------------------------
    let mut audio_recorders: Vec<AudioRecorder> = Vec::new();
    if config.summary.capture_mic {
        match AudioRecorder::start_microphone(config.mic_device.as_deref(), &config.temp_dir) {
            Ok(r) => audio_recorders.push(r),
            Err(e) => warnings.push(format!("麦克风初始化失败: {e}")),
        }
    }

    // ---- Windows system audio (WASAPI loopback) -----------------------------
    #[cfg(target_os = "windows")]
    if config.summary.capture_system_audio {
        match AudioRecorder::start_system_windows(&config.temp_dir) {
            Ok(r) => audio_recorders.push(r),
            Err(e) => warnings.push(format!("系统音频初始化失败: {e}")),
        }
    }

    // ---- elapsed timer ------------------------------------------------------
    // `active_ms` also feeds the A/V sync correction below, so it must outlive
    // the timer thread (which is detached).
    let active_ms = Arc::new(AtomicU64::new(0));
    let timer_app = app.clone();
    let timer_stop = stop_flag.clone();
    let timer_pause = pause_flag.clone();
    let timer_active = active_ms.clone();
    let timer = std::thread::spawn(move || {
        // Elapsed counts only actively-recording time.
        let mut active = Duration::ZERO;
        let mut last = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(500));
            if timer_stop.load(Ordering::SeqCst) {
                // Account for the final partial interval before exiting.
                if !timer_pause.load(Ordering::SeqCst) {
                    active += last.elapsed();
                }
                timer_active.store(active.as_millis() as u64, Ordering::SeqCst);
                break;
            }
            if !timer_pause.load(Ordering::SeqCst) {
                active += last.elapsed();
            }
            last = Instant::now();
            timer_active.store(active.as_millis() as u64, Ordering::SeqCst);
            let _ = timer_app.emit(AppEvent::Elapsed.as_str(), active.as_millis() as u64);
        }
    });

    // ---- wait for stop or unexpected capture exit --------------------------
    loop {
        if stop_flag.load(Ordering::SeqCst) || capture_handle.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if capture_handle.is_finished() && !stop_flag.load(Ordering::SeqCst) {
        warnings.push("屏幕捕获意外结束，正在完成录制".into());
    }
    *state_flag.lock() = SessionState::Stopping;

    // ---- shutdown -----------------------------------------------------------
    log::info!("[rec] shutdown: setting stop flag");
    stop_flag.store(true, Ordering::SeqCst);
    let _ = ticker_handle.join();
    log::info!("[rec] shutdown: ticker joined");
    join_with_timeout(capture_handle, "capture");
    log::info!("[rec] shutdown: capture joined");

    // cpal recorders: flush raw PCM files.
    let mut audio_sources: Vec<RawAudioSource> = audio_recorders
        .into_iter()
        .filter_map(|r| r.finish().ok())
        .collect();

    // macOS system audio: close the pipe and reuse the track only when the
    // writer flushed it completely.
    #[cfg(target_os = "macos")]
    if let Some(mut pipe) = scap_audio {
        if pipe.close() {
            let path = config.temp_dir.join("screencut-system.raw");
            if let Ok(meta) = std::fs::metadata(&path) {
                if meta.len() > 0 {
                    audio_sources.push(RawAudioSource {
                        path,
                        sample_rate: 48_000,
                        channels: 2,
                        label: "system",
                    });
                }
            }
        }
    }

    // The ticker owned the ffmpeg stdin; dropping it when the thread exited
    // closed the pipe, so ffmpeg can now finish writing the temp mp4.
    log::info!("[rec] shutdown: waiting for video encoder (pid {})", ffmpeg_child.id());
    let status = ffmpeg_child
        .wait()
        .map_err(|e| AppError::Internal(format!("ffmpeg 等待失败: {e}")))?;
    log::info!("[rec] shutdown: video encode finished: {status:?}");
    if !status.success() {
        let stderr = ffmpeg_child
            .stderr
            .take()
            .map(|mut s| {
                use std::io::Read;
                let mut buf = String::new();
                let _ = s.read_to_string(&mut buf);
                buf
            })
            .unwrap_or_default();
        log::error!("[rec] video encode failed: {stderr}");
        return Err(AppError::FfmpegFailed(status.code().unwrap_or(-1)));
    }

    // ---- A/V sync correction ------------------------------------------------
    // The raw frame pipe + encoder cannot always sustain the target fps (4K is
    // bandwidth-heavy), so the encoded video can come out shorter than the real
    // recording time while audio is captured in real time. Stretch the video to
    // the real duration so the tracks stay aligned.
    // Reference: the exact audio length when audio was recorded (real time by
    // construction), otherwise the supervisor's active-time counter.
    let audio_secs = audio_sources
        .iter()
        .filter_map(|s| {
            let bytes = std::fs::metadata(&s.path).ok()?.len();
            (s.sample_rate > 0 && s.channels > 0).then(|| {
                bytes as f64 / (s.sample_rate as f64 * s.channels as f64 * 4.0)
            })
        })
        .fold(0.0_f64, f64::max);
    let active_secs = if audio_secs > 0.0 {
        audio_secs
    } else {
        active_ms.load(Ordering::SeqCst) as f64 / 1000.0
    };
    let video_meta = ffmpeg::probe_video(&video_tmp).unwrap_or_default();
    if active_secs > 0.5 && video_meta.duration + 0.15 < active_secs {
        let factor = active_secs / video_meta.duration.max(0.001);
        log::info!(
            "[rec] retime: video {:.2}s -> {:.2}s (factor {:.3})",
            video_meta.duration, active_secs, factor
        );
        match ffmpeg::retime_video(&ffmpeg, &video_tmp, factor, fps, encoder, vw, vh) {
            Ok(()) => log::info!("[rec] retime done"),
            Err(e) => warnings.push(format!("视频时长校正失败: {e}")),
        }
    }

    // ---- final mux ----------------------------------------------------------
    let audio_used = !audio_sources.is_empty();

    let final_args = mux_args(&video_tmp, &audio_sources, &config.output_path);
    log::info!("[rec] mux: {} audio source(s)", audio_sources.len());
    let mux_out = std::process::Command::new(&ffmpeg)
        .args(&final_args)
        .output()
        .map_err(|e| AppError::Internal(format!("复用命令启动失败: {e}")))?;
    log::info!("[rec] mux done: {:?}", mux_out.status);
    if !mux_out.status.success() {
        log::error!(
            "[rec] mux failed: {}",
            String::from_utf8_lossy(&mux_out.stderr)
        );
        return Err(AppError::FfmpegFailed(mux_out.status.code().unwrap_or(-1)));
    }

    for src in audio_sources {
        let _ = std::fs::remove_file(&src.path);
    }
    let _ = std::fs::remove_file(&video_tmp);
    let _ = timer.join();
    *state_flag.lock() = SessionState::Idle;

    let meta = ffmpeg::probe_video(&config.output_path).unwrap_or_default();
    let size = std::fs::metadata(&config.output_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // A pure-black or frameless output means the capture never delivered real
    // content. Surface the most likely causes instead of leaving the user with
    // an unplayable-looking file.
    if meta.nb_frames == 0 {
        warnings.push(
            "未捕获到任何画面。可能是屏幕录制权限未生效（请在系统设置中允许后重启应用）、\
             显示器处于休眠状态，或所选窗口已被最小化/位于其他桌面。"
                .into(),
        );
    } else {
        let ratio = ffmpeg::black_ratio(&ffmpeg, &config.output_path, meta.duration);
        log::info!("[rec] black ratio: {ratio:.2}");
        if ratio > 0.98 {
            warnings.push(
                "录制内容为空白（纯黑）。常见原因：屏幕录制权限未完全生效（请退出并重新打开应用）、\
                 录制期间显示器已休眠/锁定，或所选窗口被其他窗口完全遮挡。"
                    .into(),
            );
        }
    }

    let result = RecordingResult {
        path: config.output_path.to_string_lossy().into_owned(),
        file_name: config
            .output_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size_bytes: size,
        duration_ms: (meta.duration * 1000.0) as u64,
        width: meta.width.max(video_meta.width),
        height: meta.height.max(video_meta.height),
        codec: meta.codec_name.or(video_meta.codec_name),
        frames: meta.nb_frames,
        audio: audio_used,
        warnings,
    };

    let _ = app.emit(AppEvent::Result.as_str(), &result);
    log::info!(
        "[rec] done in {:?} -> {}",
        started.elapsed(),
        result.path
    );
    Ok(result)
}

fn join_with_timeout(handle: JoinHandle<()>, name: &str) {
    const TIMEOUT: Duration = Duration::from_secs(3);
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if handle.is_finished() {
            let _ = handle.join();
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    log::warn!("[rec] {name} thread did not exit within {TIMEOUT:?}; abandoning");
}

// ---------------------------------------------------------------------------
// macOS system audio pipe: scap delivers f32le 48k stereo interleaved samples
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
struct ScapAudioPipe {
    tx: Option<mpsc::SyncSender<Vec<u8>>>,
    handle: Option<JoinHandle<()>>,
}

#[cfg(target_os = "macos")]
impl ScapAudioPipe {
    fn start(temp_dir: &PathBuf) -> AppResult<Self> {
        let path = temp_dir.join("screencut-system.raw");
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(128);
        let handle = std::thread::spawn(move || {
            let mut writer = std::io::BufWriter::new(
                std::fs::File::create(&path).expect("create system audio tmp"),
            );
            while let Ok(bytes) = rx.recv() {
                if writer.write_all(&bytes).is_err() {
                    break;
                }
            }
            let _ = writer.flush();
        });
        Ok(ScapAudioPipe { tx: Some(tx), handle: Some(handle) })
    }

    /// A cloneable handle for the capture thread.
    fn sender(&self) -> Option<mpsc::SyncSender<Vec<u8>>> {
        self.tx.clone()
    }

    /// Drop the supervisor's sender and wait briefly for the writer to flush.
    /// Returns false when the writer cannot finish (e.g. the capture thread is
    /// stuck in its own teardown and still holds a sender) so the caller can
    /// skip the track instead of deadlocking the whole shutdown.
    fn close(&mut self) -> bool {
        self.tx.take();
        let Some(handle) = self.handle.take() else {
            return true;
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if handle.is_finished() {
                let _ = handle.join();
                log::info!("[rec] system audio writer exited cleanly");
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        log::warn!("[rec] system audio writer did not exit in time; skipping the track");
        false
    }
}

// When the last Arc is dropped the sender goes away, the writer flushes and
// exits, and the raw file is complete and ready for muxing.
#[cfg(target_os = "macos")]
impl Drop for ScapAudioPipe {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_buffer_keeps_latest() {
        let buf = FrameBuffer::new();
        assert!(buf.snapshot().is_none());
        buf.store(Arc::new(FrameBlob { width: 10, height: 10, data: vec![1] }));
        assert_eq!(buf.snapshot().unwrap().data, vec![1]);
        buf.store(Arc::new(FrameBlob { width: 10, height: 10, data: vec![2] }));
        assert_eq!(buf.snapshot().unwrap().data, vec![2]);
    }

    #[test]
    fn rescale_bgr24_downsamples_and_preserves_pixels() {
        // 2x2 source, distinct colours per pixel (tightly packed BGR24).
        let src: Vec<u8> = vec![
            10, 0, 0, 20, 0, 0, //
            30, 0, 0, 40, 0, 0,
        ];
        let out = rescale_bgr24(&src, 2, 2, 1, 1);
        assert_eq!(out.len(), 3);
        // Nearest pick of (0,0) is the top-left pixel.
        assert_eq!(&out[..3], &[10, 0, 0]);
    }

    #[test]
    fn rescale_bgr24_upsamples_without_black_bars() {
        let src: Vec<u8> = vec![200, 100, 50];
        let out = rescale_bgr24(&src, 1, 1, 4, 2);
        assert_eq!(out.len(), 4 * 2 * 3);
        // Every output pixel must be the single input pixel — no zero fills.
        assert!(out.chunks_exact(3).all(|px| px == [200, 100, 50]));
    }

    #[test]
    fn session_state_serialises_snake_case() {
        assert_eq!(
            serde_json::to_string(&SessionState::Paused).unwrap(),
            "\"paused\""
        );
        assert_eq!(
            serde_json::to_string(&SessionState::Recording).unwrap(),
            "\"recording\""
        );
    }
}
