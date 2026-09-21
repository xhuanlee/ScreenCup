//! Grab a single still frame of a capture target, so the region-selection
//! overlay can render a magnifier loupe over a frozen image (like CleanShot X).
//!
//! Starting a live stream just to read one frame is wasteful and can race with
//! the overlay window appearing, so this uses scap's capturer for a brief
//! one-shot capture and PNG-encodes the result into the cache directory.

use std::fs;
use std::io::BufWriter;
use std::path::PathBuf;

use png::BitDepth;
use png::ColorType;
use scap::capturer::{Capturer, Options};
use scap::frame::{Frame, FrameType};
use scap::Target;

use crate::capture::TargetRegistry;
use crate::error::{AppError, AppResult};

/// Capture one frame of `target` and write it to the cache dir as PNG.
///
/// Returns the file path plus the physical pixel size of the image, so the
/// overlay can map logical selection coordinates to image pixels.
#[derive(Debug, serde::Serialize)]
pub struct StillFrame {
    pub path: String,
    pub width: u32,
    pub height: u32,
    /// Physical pixels per logical point on the captured display.
    pub scale_factor: f64,
}

pub fn grab_still(cache_dir: &PathBuf, target: &Target) -> AppResult<StillFrame> {
    if !scap::is_supported() || !scap::has_permission() {
        return Err(AppError::ScreenPermissionDenied);
    }

    let options = Options {
        // One still frame is enough; a high fps only makes the capturer spin.
        fps: 5,
        show_cursor: false,
        show_highlight: false,
        target: Some(target.clone()),
        crop_area: None,
        output_type: FrameType::BGRAFrame,
        output_resolution: scap::capturer::Resolution::Captured,
        excluded_targets: None,
        captures_audio: false,
        exclude_current_process_audio: false,
    };

    let mut capturer = Capturer::build(options).map_err(|e| {
        log::warn!("[still] capturer build failed: {e:?}");
        AppError::ScreenPermissionDenied
    })?;

    let [vw, vh] = capturer.get_output_frame_size();
    if vw == 0 || vh == 0 {
        return Err(AppError::Internal("无效的捕获尺寸".into()));
    }

    capturer.start_capture();

    // The first delivered frame is the freshest one available; a couple of
    // retries keep a slow first frame (display just woke) from failing.
    let mut bgra: Option<(i32, i32, Vec<u8>)> = None;
    for _ in 0..20 {
        match capturer.get_next_frame() {
            Ok(Frame::Video(scap::frame::VideoFrame::BGRA(f))) => {
                bgra = Some((f.width, f.height, f.data));
                break;
            }
            // Some engines deliver BGR0 (3 bytes/px) when BGRA is unavailable;
            // the loupe only needs *something* to show, so accept it too.
            Ok(Frame::Video(scap::frame::VideoFrame::BGR0(f))) => {
                bgra = Some((f.width, f.height, bgr0_to_bgra(&f.data)));
                break;
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    capturer.stop_capture();

    let (width, height, data) =
        bgra.ok_or_else(|| AppError::Internal("未能捕获画面".into()))?;

    let out_dir = cache_dir.join("still");
    fs::create_dir_all(&out_dir)
        .map_err(|e| AppError::Internal(format!("无法创建缓存目录: {e}")))?;
    let path = out_dir.join("region-still.png");

    encode_png(&path, width as u32, height as u32, &data)?;

    let scale = scap::get_scale_factor(target);
    log::info!(
        "[still] wrote {} ({}x{}, capture {}x{})",
        path.display(),
        width,
        height,
        vw,
        vh
    );

    Ok(StillFrame {
        path: path.to_string_lossy().into_owned(),
        width: width as u32,
        height: height as u32,
        scale_factor: if scale > 0.0 { scale } else { 1.0 },
    })
}

/// Pad a tightly-packed BGR24 buffer to BGRA, the byte order PNG's RGBA
/// encoder expects. Used only as a fallback capture format.
fn bgr0_to_bgra(bgr: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bgr.len() / 3 * 4);
    for px in bgr.chunks_exact(3) {
        out.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    out
}

/// BGRA (as scap delivers it) is exactly the byte order PNG's RGBA encoder
/// wants, so this is a straight copy.
fn encode_png(path: &PathBuf, width: u32, height: u32, bgra: &[u8]) -> AppResult<()> {
    let file = fs::File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let expected = width as usize * height as usize * 4;
    if bgra.len() < expected {
        return Err(AppError::Internal(format!(
            "画面数据不完整：{} 字节，期望 {}",
            bgra.len(),
            expected
        )));
    }
    let png_err = |e: png::EncodingError| AppError::Internal(format!("PNG 编码失败: {e}"));
    let mut writer = encoder.write_header().map_err(png_err)?;
    writer
        .write_image_data(&bgra[..expected])
        .map_err(png_err)?;
    writer.finish().map_err(png_err)?;
    Ok(())
}

/// Resolve a target id from the registry, falling back to the main display.
pub fn target_for_id(id: Option<u32>) -> AppResult<Target> {
    if let Some(id) = id {
        if let Some(t) = TargetRegistry::refresh()?.find(id).cloned() {
            return Ok(t);
        }
    }
    Ok(Target::Display(scap::get_main_display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgr0_to_bgra_preserves_pixels_and_sets_alpha() {
        // 2 pixels: blue-ish, then green-ish (BGR order).
        let bgr: Vec<u8> = vec![10, 20, 30, 40, 50, 60];
        let rgba = bgr0_to_bgra(&bgr);
        assert_eq!(rgba.len(), 8);
        assert_eq!(&rgba[..4], &[10, 20, 30, 255]);
        assert_eq!(&rgba[4..], &[40, 50, 60, 255]);
    }

    #[test]
    fn bgr0_to_bgra_handles_empty() {
        assert!(bgr0_to_bgra(&[]).is_empty());
    }

    #[test]
    fn encode_png_rejects_short_buffer() {
        let dir = test_dir();
        let path = dir.join("short.png");
        // 2x2 needs 16 bytes; give it fewer.
        let err = encode_png(&path, 2, 2, &[0u8; 8]);
        assert!(err.is_err(), "short buffers must be rejected");
    }

    #[test]
    fn encode_png_roundtrips_bgra() {
        let dir = test_dir();
        let path = dir.join("ok.png");
        // 1x1 fully opaque red in BGRA.
        let bgra: Vec<u8> = vec![0, 0, 255, 255];
        encode_png(&path, 1, 1, &bgra).expect("encode");
        let bytes = std::fs::read(&path).expect("read back");
        // PNG magic header.
        assert_eq!(&bytes[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    }

    /// A scratch dir the test process is actually allowed to write to.
    fn test_dir() -> PathBuf {
        // Tests run under a GUI-app sandbox where /tmp is not writable; the
        // crate's own target dir always is.
        let base: PathBuf = match std::env::var("CARGO_TARGET_TMPDIR") {
            Ok(d) => PathBuf::from(d),
            Err(_) => match std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from))
            {
                Some(d) => d,
                None => std::env::temp_dir(),
            },
        };
        let dir = base.join("screencut-still-test");
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        eprintln!("[still-test] using {}", dir.display());
        dir
    }
}
