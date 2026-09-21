//! End-to-end encoder pipeline test: synthetic BGR0 frames (and synthetic
//! PCM audio) are piped through the real ffmpeg binary exactly the way the
//! recorder does, then probed back with ffprobe.
//!
//! Skipped automatically when ffmpeg is not installed.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use screencut_lib::ffmpeg::{
    bitrate_for, find_ffmpeg, list_encoders, mux_args, pick_encoder, probe_video, spawn_with_stdin,
    video_record_args, RawAudioSource,
};

/// Returns None when ffmpeg is not installed, so the pipeline tests are
/// skipped instead of failing on machines without it.
fn ffmpeg_or_skip() -> Option<PathBuf> {
    let f = find_ffmpeg()?;
    if !f.exists() {
        return None;
    }
    Some(f)
}

fn temp_dir(name: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!("screencut-test-{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A synthetic 320x240 gradient that shifts every frame, so encoders cannot
/// compress it to nothing.
fn frame(w: u32, h: u32, index: usize) -> Vec<u8> {
    // Tightly packed BGR24, matching what scap delivers and what we declare
    // to ffmpeg.
    let mut buf = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = index as u32;
            buf.push((x as u32 % 128 + t % 64) as u8); // B
            buf.push((y as u32 % 128 + t % 96) as u8); // G
            buf.push(((x + y) as u32 % 200 + t % 32) as u8); // R
        }
    }
    buf
}

fn write_frames(child: &mut std::process::Child, w: u32, h: u32, _fps: u32, count: usize) {
    let stdin = child.stdin.as_mut().expect("no stdin");
    for i in 0..count {
        stdin.write_all(&frame(w, h, i)).unwrap();
        stdin.flush().unwrap();
    }
    let _ = stdin.flush();
    drop(child.stdin.take());
}

fn run(child: &mut std::process::Child) {
    let status = child.wait().expect("ffmpeg did not run");
    assert!(status.success(), "ffmpeg failed: {status:?}");
}

fn encoders(ffmpeg: &Path) -> HashSet<String> {
    list_encoders(ffmpeg).unwrap_or_default()
}

#[test]
fn encodes_synthetic_frames_and_probes_back() {
    let Some(ffmpeg) = ffmpeg_or_skip() else {
        eprintln!("skipped: ffmpeg not found");
        return;
    };
    let dir = temp_dir("video");
    let out = dir.join("video.mp4");

    let (w, h, fps, frames) = (320_u32, 240_u32, 30_u32, 60_usize);
    let encoder = pick_encoder(&encoders(&ffmpeg));
    let args = video_record_args(encoder, w, h, fps, &out);
    let mut child = spawn_with_stdin(&ffmpeg, &args).expect("spawn ffmpeg");

    write_frames(&mut child, w, h, fps, frames);
    run(&mut child);

    let meta = probe_video(&out).expect("probe");
    assert_eq!(meta.width, w, "width: {:?}", meta);
    assert_eq!(meta.height, h, "height: {:?}", meta);
    // 60 frames at 30 fps should be very close to 2 seconds.
    assert!(meta.duration > 1.0 && meta.duration < 3.0, "duration: {:?}", meta);
    assert!(meta.nb_frames >= 55, "nb_frames: {:?}", meta);
    assert!(meta.codec_name.is_some());
    println!("pipeline ok: {meta:?}");
}

#[test]
fn hardware_encoder_or_libx264_selected() {
    let Some(ffmpeg) = ffmpeg_or_skip() else {
        eprintln!("skipped: ffmpeg not found");
        return;
    };
    let set = encoders(&ffmpeg);
    let encoder = pick_encoder(&set);
    match encoder {
        screencut_lib::ffmpeg::EncoderKind::Hardware(name) => {
            assert!(set.contains(name), "picked unavailable encoder {name}");
        }
        screencut_lib::ffmpeg::EncoderKind::Software => {
            assert!(set.contains("libx264"), "software fallback needs libx264");
        }
    }
}

#[test]
fn muxes_two_audio_streams_without_video_loss() {
    let Some(ffmpeg) = ffmpeg_or_skip() else {
        eprintln!("skipped: ffmpeg not found");
        return;
    };
    let dir = temp_dir("mux");
    let video = dir.join("v.mp4");
    let out = dir.join("out.mp4");

    // 1 second of synthetic video.
    let (w, h, fps, frames) = (160_u32, 120_u32, 30_u32, 30_usize);
    let encoder = pick_encoder(&encoders(&ffmpeg));
    let mut child =
        spawn_with_stdin(&ffmpeg, &video_record_args(encoder, w, h, fps, &video)).unwrap();
    write_frames(&mut child, w, h, fps, frames);
    run(&mut child);

    // Two synthetic 1-second 48 kHz stereo f32le files (a sine and a sweep).
    let mic = dir.join("mic.raw");
    let sys = dir.join("sys.raw");
    let tone = |freq: f64| {
        let mut buf = Vec::new();
        for i in 0..48_000 {
            let t = i as f64 / 48_000.0;
            // f32le is what mux_args declares — writing f64 bytes here would
            // deserialise into garbage / NaN.
            let s: f32 = ((2.0 * std::f64::consts::PI * freq * t).sin() * 0.2) as f32;
            for _ in 0..2 {
                buf.extend_from_slice(&s.to_le_bytes());
            }
        }
        buf
    };
    std::fs::write(&mic, tone(440.0)).unwrap();
    std::fs::write(&sys, tone(880.0)).unwrap();

    let audio = vec![
        RawAudioSource { path: mic.clone(), sample_rate: 48_000, channels: 2, label: "mic" },
        RawAudioSource { path: sys.clone(), sample_rate: 48_000, channels: 2, label: "sys" },
    ];
    let args = mux_args(&video, &audio, &out);
    // The amix filter must reference both inputs.
    assert!(args
        .iter()
        .any(|a| a.contains("amix=inputs=2:duration=longest")));

    let mut child = std::process::Command::new(&ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn mux");
    run(&mut child);

    let meta = probe_video(&out).expect("probe");
    assert_eq!(meta.width, w);
    assert_eq!(meta.height, h);
    assert!(meta.duration > 0.8, "duration: {:?}", meta);
    println!("mux ok: {meta:?}");
}

#[test]
fn bitrate_stays_within_bounds() {
    let lo = bitrate_for(640, 480, 15);
    let mid = bitrate_for(1920, 1080, 30);
    let hi = bitrate_for(3840, 2160, 60);
    assert!(lo >= 2 * 512 && lo <= 40 * 1024);
    assert!(mid >= lo && hi >= mid);
    assert!(hi <= 40 * 1024);
}

#[test]
fn black_ratio_distinguishes_blank_from_real_content() {
    use screencut_lib::ffmpeg::black_ratio;

    let Some(ffmpeg) = ffmpeg_or_skip() else {
        eprintln!("skipped: ffmpeg not found");
        return;
    };
    let dir = temp_dir("black");

    // Two seconds of pure black.
    let black = dir.join("black.mp4");
    run(&mut std::process::Command::new(&ffmpeg)
        .args([
            "-hide_banner", "-loglevel", "error", "-y",
            "-f", "lavfi", "-i", "color=c=black:s=320x240:r=30:d=2",
            "-pix_fmt", "yuv420p", black.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn black"));
    assert!(black_ratio(&ffmpeg, &black, 2.0) > 0.98, "black file should read as black");

    // Two seconds of moving test content.
    let real = dir.join("real.mp4");
    run(&mut std::process::Command::new(&ffmpeg)
        .args([
            "-hide_banner", "-loglevel", "error", "-y",
            "-f", "lavfi", "-i", "testsrc=s=320x240:r=30:d=2",
            "-pix_fmt", "yuv420p", real.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn real"));
    assert!(black_ratio(&ffmpeg, &real, 2.0) < 0.1, "content file should not read as black");
}

#[test]
fn retiming_stretches_video_to_reference_duration() {
    use screencut_lib::ffmpeg::retime_video;

    let Some(ffmpeg) = ffmpeg_or_skip() else {
        eprintln!("skipped: ffmpeg not found");
        return;
    };
    let dir = temp_dir("retime");
    let video = dir.join("v.mp4");

    // 1 second of 30 fps video.
    let (w, h, fps, frames) = (160_u32, 120_u32, 30_u32, 30_usize);
    let encoder = pick_encoder(&encoders(&ffmpeg));
    let mut child =
        spawn_with_stdin(&ffmpeg, &video_record_args(encoder, w, h, fps, &video)).unwrap();
    write_frames(&mut child, w, h, fps, frames);
    run(&mut child);

    let before = probe_video(&video).expect("probe before");
    assert!((before.duration - 1.0).abs() < 0.2, "baseline duration: {before:?}");

    // Simulate the recorder falling behind real time: stretch by 2x.
    retime_video(&ffmpeg, &video, 2.0, fps, encoder, w, h).expect("retime");

    let after = probe_video(&video).expect("probe after");
    assert_eq!(after.width, w);
    assert_eq!(after.height, h);
    assert!(
        (after.duration - 2.0).abs() < 0.3,
        "retime should land near 2s: {after:?}"
    );
    assert!(after.nb_frames > before.nb_frames, "frames should be padded: {after:?}");
    println!("retime ok: {before:?} -> {after:?}");
}
