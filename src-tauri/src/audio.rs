//! Audio capture with `cpal`:
//! - Microphones on macOS (CoreAudio) and Windows (WASAPI).
//! - Windows system audio via WASAPI loopback (building an input stream on a
//!   render endpoint transparently enables loopback in cpal).
//!
//! All captured PCM is normalised to interleaved f32le and streamed to a temp
//! file through a writer thread so the realtime audio callback never blocks on
//! disk.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};

use crate::error::{AppError, AppResult};
use crate::ffmpeg::RawAudioSource;

/// A short-lived audio pipeline: realtime callback -> writer thread -> raw file.
pub struct AudioRecorder {
    kind: &'static str,
    stream: Option<Stream>,
    sender: Option<SyncSender<Packet>>,
    writer: Option<JoinHandle<()>>,
    sample_rate: u32,
    channels: u16,
    out_path: PathBuf,
}

enum Packet {
    Bytes(Vec<u8>),
    /// Signifies end of stream; the writer thread flushes and exits.
    End,
}

/// Find an input device by name, falling back to the system default.
fn pick_input_device(name: Option<&str>) -> AppResult<cpal::Device> {
    let host = cpal::default_host();
    let device = match name {
        Some(wanted) if !wanted.is_empty() => host
            .input_devices()?
            .find(|d| d.name().ok().as_deref() == Some(wanted))
            .or_else(|| host.default_input_device()),
        _ => host.default_input_device(),
    };
    device.ok_or_else(|| AppError::AudioDeviceUnavailable("未找到麦克风设备".into()))
}

/// Choose a config, preferring 48 kHz f32. Returns the config together with
/// the sample format the resulting stream will deliver (cpal's StreamConfig
/// does not carry it).
fn pick_input_config(device: &cpal::Device) -> AppResult<(StreamConfig, SampleFormat)> {
    let mut configs = device.supported_input_configs()?;
    // Prefer f32 @ 48kHz, any channel count.
    let preferred = configs.find(|c| {
        c.sample_format() == SampleFormat::F32
            && c.min_sample_rate().0 <= 48_000
            && c.max_sample_rate().0 >= 48_000
    });
    let (cfg, format) = if let Some(c) = preferred {
        let format = c.sample_format();
        (c.with_sample_rate(cpal::SampleRate(48_000)), format)
    } else {
        let d = device.default_input_config()?;
        let format = d.sample_format();
        (d, format)
    };
    Ok((cfg.into(), format))
}

impl AudioRecorder {
    /// Start capturing a microphone (macOS / Windows).
    pub fn start_microphone(name: Option<&str>, temp_dir: &Path) -> AppResult<Self> {
        let device = pick_input_device(name)?;
        let (config, format) = pick_input_config(&device)?;
        Self::start_stream(device, config, format, "mic", temp_dir)
    }

    /// Start capturing system audio. On macOS this is handled by scap's
    /// ScreenCaptureKit stream, so this is only used on Windows.
    #[cfg(target_os = "windows")]
    pub fn start_system_windows(temp_dir: &Path) -> AppResult<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AppError::AudioDeviceUnavailable("未找到音频输出设备".into()))?;
        // The render endpoint's shared format is exactly what loopback delivers.
        let cfg = device.default_output_config()?;
        let format = cfg.sample_format();
        Self::start_stream(device, cfg.into(), format, "system", temp_dir)
    }

    fn start_stream(
        device: cpal::Device,
        config: StreamConfig,
        sample_format: SampleFormat,
        kind: &'static str,
        temp_dir: &Path,
    ) -> AppResult<Self> {
        let sample_rate = config.sample_rate.0;
        let channels = config.channels;
        let out_path = temp_dir.join(format!("screencut-{kind}.raw"));

        let (tx, rx) = mpsc::sync_channel::<Packet>(128);
        let writer = spawn_writer(rx, &out_path)?;

        // Build a callback matching the concrete sample format: cpal panics if
        // the callback's slice type disagrees with the config.
        let timeout = Some(Duration::from_secs(2));
        let stream = match sample_format {
            SampleFormat::F32 => {
                let on_error = on_stream_error(kind, tx.clone());
                let data_tx = tx.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _| send_samples(data, convert_f32, &data_tx),
                    on_error,
                    timeout,
                )
            }
            SampleFormat::I16 => {
                let on_error = on_stream_error(kind, tx.clone());
                let data_tx = tx.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _| send_samples(data, convert_i16, &data_tx),
                    on_error,
                    timeout,
                )
            }
            SampleFormat::U16 => {
                let on_error = on_stream_error(kind, tx.clone());
                let data_tx = tx.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _| send_samples(data, convert_u16, &data_tx),
                    on_error,
                    timeout,
                )
            }
            other => {
                return Err(AppError::AudioDeviceUnavailable(format!(
                    "不支持的音频采样格式: {other:?}"
                )))
            }
        }
        .map_err(|e| {
            AppError::AudioDeviceUnavailable(format!("无法启动音频流: {e}"))
        })?;

        stream.play().map_err(|e| {
            AppError::AudioDeviceUnavailable(format!("无法启动音频流: {e}"))
        })?;

        log::info!(
            "[audio:{kind}] started: {sample_rate} Hz, {channels} ch, {:?}",
            sample_format
        );

        Ok(AudioRecorder {
            kind,
            stream: Some(stream),
            sender: Some(tx),
            writer: Some(writer),
            sample_rate,
            channels,
            out_path,
        })
    }

    /// Flush and finalise; returns the raw PCM source descriptor.
    pub fn finish(mut self) -> AppResult<RawAudioSource> {
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
            drop(stream);
        }
        if let Some(tx) = self.sender.take() {
            let _ = tx.send(Packet::End);
        }
        if let Some(handle) = self.writer.take() {
            let _ = handle.join();
        }
        log::info!("[audio:{}] finished -> {}", self.kind, self.out_path.display());
        Ok(RawAudioSource {
            path: self.out_path,
            sample_rate: self.sample_rate,
            channels: self.channels,
            label: self.kind,
        })
    }
}

/// Copy PCM samples to the writer thread as little-endian f32, converting
/// integer formats the way dasp does (no generic trait bounds needed).
fn send_samples<T: Copy>(data: &[T], convert: fn(T) -> f32, tx: &SyncSender<Packet>) {
    let mut bytes = Vec::with_capacity(data.len() * 4);
    for sample in data {
        bytes.extend_from_slice(&convert(*sample).to_le_bytes());
    }
    if tx.send(Packet::Bytes(bytes)).is_err() {
        log::error!("[audio] writer thread died; samples dropped");
    }
}

fn convert_f32(s: f32) -> f32 {
    s
}

fn convert_i16(s: i16) -> f32 {
    s as f32 / 32_768.0
}

fn convert_u16(s: u16) -> f32 {
    (s as f32 - 32_768.0) / 32_768.0
}

fn on_stream_error(
    kind: &'static str,
    tx: SyncSender<Packet>,
) -> impl FnMut(cpal::StreamError) + Send + 'static {
    move |err| {
        log::error!("[audio:{kind}] stream error: {err:?}");
        let _ = tx.send(Packet::End);
    }
}

fn spawn_writer(rx: mpsc::Receiver<Packet>, path: &Path) -> AppResult<JoinHandle<()>> {
    let path = path.to_path_buf();
    let file = File::create(&path).map_err(|e| {
        AppError::Internal(format!("无法创建音频临时文件 {}: {e}", path.display()))
    })?;
    let mut writer = BufWriter::new(file);
    Ok(std::thread::spawn(move || {
        let mut total = 0usize;
        while let Ok(packet) = rx.recv() {
            match packet {
                Packet::Bytes(bytes) => {
                    if let Err(e) = writer.write_all(&bytes) {
                        log::error!("[audio] write failed: {e}");
                        return;
                    }
                    total += bytes.len();
                }
                Packet::End => break,
            }
        }
        let _ = writer.flush();
        log::debug!("[audio] wrote {} bytes to {}", total, path.display());
    }))
}

/// Sample a device list for the UI: (name, is_default).
pub fn list_microphone_names() -> Vec<String> {
    let host = match cpal::default_host().input_devices() {
        Ok(devices) => devices,
        Err(_) => return vec![],
    };
    let mut names = Vec::new();
    for d in host {
        if let Ok(name) = d.name() {
            if !name.trim().is_empty() && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// Length in bytes of `n` interleaved samples of `channels` f32 channels.
#[cfg(test)]
pub(crate) fn pcm_bytes(n: usize, channels: u16) -> usize {
    n * channels as usize * std::mem::size_of::<f32>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_bytes_math() {
        assert_eq!(pcm_bytes(480, 2), 480 * 2 * 4);
    }
}
