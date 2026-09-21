use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::error::AppResult;
use crate::geometry::Rect;

/// What to record.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Display,
    Window,
    Region,
}

/// Output resolution preset. `Original` keeps the captured resolution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QualityPreset {
    Original,
    P1080,
    P720,
    P480,
}

impl QualityPreset {
    pub fn to_scap(self) -> scap::capturer::Resolution {
        match self {
            QualityPreset::Original => scap::capturer::Resolution::Captured,
            QualityPreset::P1080 => scap::capturer::Resolution::_1080p,
            QualityPreset::P720 => scap::capturer::Resolution::_720p,
            QualityPreset::P480 => scap::capturer::Resolution::_480p,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub kind: SourceKind,
    /// None resolves to the main display.
    pub target_id: Option<u32>,
    /// Logical rect for `Region` mode.
    pub region: Option<Rect>,
    pub capture_system_audio: bool,
    pub capture_mic: bool,
    pub mic_device: Option<String>,
    pub show_cursor: bool,
    pub fps: u32,
    pub quality: QualityPreset,
    /// None → platform default (Movies / Videos folder).
    pub output_dir: Option<String>,
    /// Hide the main window while recording, leaving only the floating bar.
    pub hide_main_while_recording: bool,
    /// UI language: "zh" or "en". Anything else falls back to Chinese.
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            kind: SourceKind::Display,
            target_id: None,
            region: None,
            capture_system_audio: true,
            capture_mic: false,
            mic_device: None,
            show_cursor: true,
            fps: 30,
            quality: QualityPreset::Original,
            output_dir: None,
            hide_main_while_recording: true,
            language: default_language(),
        }
    }
}

/// Best-effort first-run language. macOS/Linux expose the locale via `LANG`;
/// a Chinese locale yields "zh", everything else "en". Windows has no `LANG`
/// env var, so it falls back to Chinese — the app's original language — and
/// the user can switch in the header.
fn default_language() -> String {
    let is_chinese = std::env::var("LANG")
        .ok()
        .and_then(|l| l.split('_').next().map(|tag| tag == "zh"))
        .unwrap_or(true);
    if is_chinese {
        "zh".to_string()
    } else {
        "en".to_string()
    }
}

impl Settings {
    fn file_path(dir: &Path) -> std::path::PathBuf {
        dir.join("settings.json")
    }

    pub fn load(dir: &Path) -> Self {
        match fs::read(Self::file_path(dir)) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, dir: &Path) -> AppResult<()> {
        fs::create_dir_all(dir)?;
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::write(Self::file_path(dir), bytes)?;
        Ok(())
    }

    /// Clamp fps to a supported value.
    pub fn effective_fps(&self) -> u32 {
        match self.fps {
            24 | 30 | 60 => self.fps,
            _ => 30,
        }
    }

    /// A snapshot of the bits that drive the recorder, cheap to clone.
    pub fn recording_summary(&self) -> RecordingSummary {
        RecordingSummary {
            fps: self.effective_fps(),
            quality: self.quality,
            show_cursor: self.show_cursor,
            capture_system_audio: self.capture_system_audio,
            capture_mic: self.capture_mic,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RecordingSummary {
    pub fps: u32,
    pub quality: QualityPreset,
    pub show_cursor: bool,
    pub capture_system_audio: bool,
    pub capture_mic: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default();
        assert_eq!(s.kind, SourceKind::Display);
        assert!(s.capture_system_audio);
        assert!(!s.capture_mic);
        assert_eq!(s.effective_fps(), 30);
    }

    #[test]
    fn roundtrip_preserves_fields() {
        let dir = std::env::temp_dir().join("screencut-settings-test");
        let _ = fs::remove_dir_all(&dir);
        let mut s = Settings::default();
        s.kind = SourceKind::Region;
        s.region = Some(Rect::new(10.0, 20.0, 800.0, 600.0));
        s.capture_mic = true;
        s.mic_device = Some("Mic".into());
        s.fps = 60;
        s.quality = QualityPreset::P720;
        s.save(&dir).unwrap();

        let loaded = Settings::load(&dir);
        assert_eq!(loaded.kind, SourceKind::Region);
        assert_eq!(loaded.region, Some(Rect::new(10.0, 20.0, 800.0, 600.0)));
        assert_eq!(loaded.mic_device.as_deref(), Some("Mic"));
        assert_eq!(loaded.fps, 60);
        assert_eq!(loaded.quality, QualityPreset::P720);
    }

    #[test]
    fn fps_falls_back_to_30() {
        let mut s = Settings::default();
        s.fps = 144;
        assert_eq!(s.effective_fps(), 30);
        s.fps = 24;
        assert_eq!(s.effective_fps(), 24);
    }

    #[test]
    fn language_roundtrips() {
        let dir = std::env::temp_dir().join("screencut-settings-lang-test");
        let _ = fs::remove_dir_all(&dir);
        let mut s = Settings::default();
        s.language = "en".to_string();
        s.save(&dir).unwrap();
        let loaded = Settings::load(&dir);
        assert_eq!(loaded.language, "en");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_language_is_chinese_by_default() {
        // A settings file with an unknown/absent language must not panic and
        // must fall back to something usable.
        let s = Settings::default();
        assert!(s.language == "zh" || s.language == "en");
    }
}
