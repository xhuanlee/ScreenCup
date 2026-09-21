//! ffmpeg subprocess pipeline: locating the binary, choosing an encoder and
//! building the argument vectors used for recording / muxing / probing.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Locates an ffmpeg executable.
pub fn find_ffmpeg() -> Option<PathBuf> {
    // 1. explicit override
    if let Ok(p) = std::env::var("SCREENCUT_FFMPEG") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }

    // 2. PATH lookup
    if let Some(p) = which_ffmpeg() {
        return Some(p);
    }

    // 3. well-known Homebrew locations (macOS dev machines)
    for cand in [
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
    ] {
        if Path::new(cand).is_file() {
            return Some(PathBuf::from(cand));
        }
    }

    None
}

fn which_ffmpeg() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let cand = dir.join(if cfg!(target_os = "windows") {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        });
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderKind {
    /// Named hardware encoder (e.g. `h264_videotoolbox`, `h264_qsv`).
    Hardware(&'static str),
    /// Software fallback.
    Software,
}

impl EncoderKind {
    pub fn name(&self) -> &'static str {
        match self {
            EncoderKind::Hardware(n) => n,
            EncoderKind::Software => "libx264",
        }
    }

    pub fn is_hardware(&self) -> bool {
        matches!(self, EncoderKind::Hardware(_))
    }
}

/// Choose the best available H.264 encoder for the current platform.
pub fn pick_encoder(available: &HashSet<String>) -> EncoderKind {
    let preferred: &[&str] = if cfg!(target_os = "macos") {
        &["h264_videotoolbox"]
    } else if cfg!(target_os = "windows") {
        &["h264_nvenc", "h264_qsv", "h264_amf"]
    } else {
        &[]
    };

    for &name in preferred {
        if available.contains(name) {
            return EncoderKind::Hardware(name);
        }
    }
    EncoderKind::Software
}

/// Parse `ffmpeg -encoders` output into a set of encoder names.
pub fn parse_encoders(output: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    for line in output.lines().skip(1) {
        // Lines look like: ` V..... h264_videotoolbox  VideoToolbox H.264 encoder`
        let mut it = line.split_whitespace();
        let flags = it.next().unwrap_or("");
        if !flags.starts_with('V') {
            continue;
        }
        if let Some(name) = it.next() {
            if name != "Name" && !name.starts_with('-') {
                set.insert(name.to_string());
            }
        }
    }
    set
}

/// Query ffmpeg for its available encoders.
pub fn list_encoders(ffmpeg: &Path) -> AppResult<HashSet<String>> {
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()
        .map_err(|_| AppError::FfmpegNotFound)?;
    Ok(parse_encoders(&String::from_utf8_lossy(&output.stdout)))
}

/// Rough target bitrate for a given resolution / framerate.
pub fn bitrate_for(width: u32, height: u32, fps: u32) -> u32 {
    let pixels = width as f64 * height as f64;
    // bits per pixel per frame — screen content compresses well but has sharp text
    let bpp = 0.09 * (fps as f64 / 30.0).max(0.5);
    let bps = pixels * bpp * fps as f64;
    // clamp to a sane window (2 Mbps .. 40 Mbps), round to 512k
    let mbps = (bps / 1_000_000.0).clamp(2.0, 40.0);
    (mbps * 2.0).round() as u32 * 512
}

/// Arguments for the live video encoding process (raw BGR0 frames on stdin).
pub fn video_record_args(
    encoder: EncoderKind,
    width: u32,
    height: u32,
    fps: u32,
    output: &Path,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-nostdin".into(),
        "-y".into(),
        // input: raw BGR0 frames arriving on stdin
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        // scap's BGR0 frame data has already been alpha-stripped to 3 bytes
        // per pixel (B,G,R) — declaring bgr0 (4 bytes/px) here would misalign
        // the byte stream and roll the picture frame over frame.
        "bgr24".into(),
        "-s".into(),
        format!("{width}x{height}"),
        "-r".into(),
        fps.to_string(),
        "-i".into(),
        "pipe:0".into(),
    ];

    if encoder.is_hardware() {
        args.extend([
            "-c:v".into(),
            encoder.name().into(),
            "-b:v".into(),
            format!("{}k", bitrate_for(width, height, fps)),
        ]);
    } else {
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "veryfast".into(),
            "-crf".into(),
            "20".into(),
        ]);
    }

    args.extend([
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-r".into(),
        fps.to_string(),
        "-movflags".into(),
        "+faststart".into(),
        "-map_metadata".into(),
        "0".into(),
        output.to_string_lossy().into_owned(),
    ]);
    args
}

/// A decoded raw PCM audio source (f32le, interleaved) captured during recording.
#[derive(Debug, Clone)]
pub struct RawAudioSource {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub channels: u16,
    pub label: &'static str,
}

/// Arguments for the final mux: combine the silent video with any captured audio
/// tracks and write the user-facing MP4.
pub fn mux_args(
    video: &Path,
    audio: &[RawAudioSource],
    output: &Path,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        video.to_string_lossy().into_owned(),
    ];

    for src in audio {
        args.extend([
            "-f".into(),
            "f32le".into(),
            "-ar".into(),
            src.sample_rate.to_string(),
            "-ac".into(),
            src.channels.to_string(),
            "-i".into(),
            src.path.to_string_lossy().into_owned(),
        ]);
    }

    match audio.len() {
        0 => {
            args.extend(["-map".into(), "0:v".into()]);
        }
        1 => {
            args.extend(["-map".into(), "0:v".into(), "-map".into(), "1:a:0".into()]);
        }
        _ => {
            let mut filter = String::new();
            for (i, _) in audio.iter().enumerate() {
                if !filter.is_empty() {
                    filter.push_str(";");
                }
                filter.push_str(&format!("[{}:a]aresample=async=1:first_pts=0[a{}]", i + 1, i));
            }
            let mix: Vec<String> = audio
                .iter()
                .enumerate()
                .map(|(i, _)| format!("[a{}]", i))
                .collect();
            filter.push_str(&format!(";{}amix=inputs={}:duration=longest:dropout_transition=0[aout]", mix.join(""), audio.len()));
            args.extend([
                "-filter_complex".into(),
                filter,
                "-map".into(),
                "0:v".into(),
                "-map".into(),
                "[aout]".into(),
            ]);
        }
    }

    args.extend([
        "-c:v".into(),
        "copy".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-ac".into(),
        "2".into(),
        "-movflags".into(),
        "+faststart".into(),
        output.to_string_lossy().into_owned(),
    ]);
    args
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VideoMeta {
    pub codec_name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub nb_frames: i64,
}

/// Probe a media file with ffprobe and return the first video stream metadata.
pub fn probe_video(path: &Path) -> AppResult<VideoMeta> {
    let ffmpeg = find_ffmpeg().ok_or(AppError::FfmpegNotFound)?;
    let ffprobe = ffmpeg.with_file_name("ffprobe");

    let out = Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height,nb_frames,duration",
            "-of",
            "json",
            &path.to_string_lossy(),
        ])
        .output()
        .map_err(|_| AppError::FfmpegNotFound)?;

    if !out.status.success() {
        return Ok(VideoMeta::default());
    }

    #[derive(Deserialize)]
    struct ProbeOut {
        streams: Vec<ProbeStream>,
    }
    #[derive(Deserialize)]
    struct ProbeStream {
        codec_name: Option<String>,
        width: Option<u32>,
        height: Option<u32>,
        nb_frames: Option<String>,
        duration: Option<String>,
    }

    let parsed: ProbeOut = serde_json::from_slice(&out.stdout).unwrap_or(ProbeOut { streams: vec![] });
    let s = parsed.streams.into_iter().next().unwrap_or(ProbeStream {
        codec_name: None,
        width: None,
        height: None,
        nb_frames: None,
        duration: None,
    });

    Ok(VideoMeta {
        codec_name: s.codec_name,
        width: s.width.unwrap_or(0),
        height: s.height.unwrap_or(0),
        duration: s.duration.and_then(|d| d.parse().ok()).unwrap_or(0.0),
        nb_frames: s
            .nb_frames
            .and_then(|n| n.parse().ok())
            .unwrap_or(0),
    })
}

/// Fraction of the video that ffmpeg's `blackdetect` filter reports as black.
/// Used to turn a silently-broken recording (all-black frames) into an
/// actionable message instead of leaving the user staring at a black file.
pub fn black_ratio(ffmpeg: &Path, path: &Path, duration: f64) -> f64 {
    if duration <= 0.0 {
        return 0.0;
    }
    let out = match Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-v",
            "info",
            "-i",
            &path.to_string_lossy(),
            "-vf",
            "blackdetect=d=0.05:pix_th=0.10",
            "-an",
            "-f",
            "null",
            "-",
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return 0.0,
    };
    let stderr = String::from_utf8_lossy(&out.stderr);
    let black_secs: f64 = stderr
        .lines()
        .filter_map(|l| {
            l.split("black_duration:")
                .nth(1)
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|v| v.parse::<f64>().ok())
        })
        .sum();
    (black_secs / duration).max(0.0).clamp(0.0, 1.0)
}

/// Stretch a finished temp video by `factor` so it matches the real recording
/// duration. Used when the capture/encode pipeline could not sustain the
/// target fps (heavy at 4K) while the audio was recorded in real time —
/// without this the tracks drift apart. Re-encodes with the same encoder kind.
pub fn retime_video(
    ffmpeg: &Path,
    video: &Path,
    factor: f64,
    fps: u32,
    encoder: EncoderKind,
    width: u32,
    height: u32,
) -> AppResult<()> {
    let staged = video.with_extension("retime.mp4");
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        video.to_string_lossy().into_owned(),
        "-vf".into(),
        format!("setpts={factor:.6}*PTS"),
        "-r".into(),
        fps.to_string(),
        "-an".into(),
    ];
    if encoder.is_hardware() {
        args.extend([
            "-c:v".into(),
            encoder.name().into(),
            "-b:v".into(),
            format!("{}k", bitrate_for(width, height, fps)),
        ]);
    } else {
        args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "veryfast".into(),
            "-crf".into(),
            "20".into(),
        ]);
    }
    args.push(staged.to_string_lossy().into_owned());

    let out = std::process::Command::new(ffmpeg)
        .args(&args)
        .output()
        .map_err(|_| AppError::FfmpegNotFound)?;
    if !out.status.success() {
        return Err(AppError::FfmpegFailed(out.status.code().unwrap_or(-1)));
    }
    std::fs::rename(&staged, video).map_err(|e| AppError::Internal(format!("视频校正文件替换失败: {e}")))?;
    Ok(())
}

/// Convenience wrapper that spawns ffmpeg with piped stdin.
pub fn spawn_with_stdin(ffmpeg: &Path, args: &[String]) -> AppResult<std::process::Child> {
    Command::new(ffmpeg)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| AppError::FfmpegNotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_encoders_picks_video_encoders() {
        let sample = "\
Encoder entries: count=3
 V..... h264_videotoolbox  VideoToolbox H.264 encoder [codec]
 V..... libx264            libx264 H.264 (codec h264)
 A..... aac_at             aac_at AAC (Advanced Audio Coding) (codec aac)
 V..... h264_qsv           h264_qsv H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10 (codec h264)
";
        let set = parse_encoders(sample);
        assert!(set.contains("h264_videotoolbox"));
        assert!(set.contains("libx264"));
        assert!(set.contains("h264_qsv"));
        assert!(!set.contains("aac_at"));
        assert!(!set.contains("Name"));
    }

    #[test]
    fn pick_prefers_hardware() {
        let mut set = HashSet::new();
        set.insert("libx264".to_string());
        assert_eq!(pick_encoder(&set), EncoderKind::Software);

        if cfg!(target_os = "macos") {
            set.insert("h264_videotoolbox".to_string());
            assert_eq!(
                pick_encoder(&set),
                EncoderKind::Hardware("h264_videotoolbox")
            );
        }
    }

    #[test]
    fn bitrate_is_monotonic_and_clamped() {
        let small = bitrate_for(640, 480, 30);
        let big = bitrate_for(3840, 2160, 60);
        assert!(small >= 2 * 512);
        assert!(big <= 40 * 1024);
        assert!(big > small);
    }

    #[test]
    fn video_record_args_shape() {
        let args = video_record_args(
            EncoderKind::Hardware("h264_videotoolbox"),
            1920,
            1080,
            30,
            Path::new("/tmp/out.mp4"),
        );
        assert!(args.contains(&"-pix_fmt".into()));
        assert!(args.contains(&"bgr24".into()));
        assert!(args.contains(&"1920x1080".into()));
        assert!(args.contains(&"h264_videotoolbox".into()));
        assert!(args.contains(&"+faststart".into()));
        assert_eq!(args.last().unwrap(), "/tmp/out.mp4");
    }

    #[test]
    fn mux_args_single_audio() {
        let audio = vec![RawAudioSource {
            path: PathBuf::from("/tmp/mic.raw"),
            sample_rate: 48000,
            channels: 2,
            label: "mic",
        }];
        let args = mux_args(Path::new("/tmp/v.mp4"), &audio, Path::new("/tmp/out.mp4"));
        assert!(args.contains(&"48000".into()));
        assert!(args.contains(&"f32le".into()));
        assert!(args.contains(&"-map".into()));
        assert!(args.contains(&"[aout]".into()) == false); // single source has no amix
        assert!(args.contains(&"copy".into()));
    }

    #[test]
    fn mux_args_two_audio_uses_amix() {
        let audio = vec![
            RawAudioSource {
                path: PathBuf::from("/tmp/mic.raw"),
                sample_rate: 48000,
                channels: 2,
                label: "mic",
            },
            RawAudioSource {
                path: PathBuf::from("/tmp/sys.raw"),
                sample_rate: 48000,
                channels: 2,
                label: "sys",
            },
        ];
        let args = mux_args(Path::new("/tmp/v.mp4"), &audio, Path::new("/tmp/out.mp4"));
        assert!(args.iter().any(|a| a.starts_with("[1:a]")));
        assert!(args
            .iter()
            .any(|a| a.contains("amix=inputs=2:duration=longest:dropout_transition=0[aout]")));
    }
}
