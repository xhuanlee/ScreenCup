//! Screen capture source management, built on `scap`.
//!
//! `scap` unifies macOS (ScreenCaptureKit) and Windows (Graphics Capture)
//! behind one API. Notable behaviours discovered while building this app:
//! - `get_all_targets()` **panics** when screen-recording permission is
//!   missing, so enumeration is guarded.
//! - `crop_area` is expressed in *logical* points (macOS) / DIPs (Windows);
//!   the resulting frames are `crop * scale_factor` pixels.
//! - Frames keep flowing on a static screen (~fps), but `stop_capture()` does
//!   NOT unblock `get_next_frame()`, so the capture thread polls a shared stop
//!   flag between frames and exits on its own.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::geometry::Rect;
use crate::settings::{QualityPreset, SourceKind};

/// A capture source chosen by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpec {
    pub kind: SourceKind,
    /// scap target id (display or window).
    pub target_id: u32,
    /// Logical rect, only used for `Region` mode.
    pub region: Option<Rect>,
}

/// Serialisable info about a capture target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureTargetInfo {
    pub id: u32,
    pub kind: String,
    pub title: String,
    pub is_primary: bool,
    /// Logical width / height.
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

/// Holds the live scap targets so a previously-picked id can be resolved.
pub struct TargetRegistry {
    items: Vec<(CaptureTargetInfo, scap::Target)>,
}

impl TargetRegistry {
    /// An empty registry (used before the first enumeration).
    pub fn empty() -> Self {
        Self { items: Vec::new() }
    }

    /// Enumerate all captureable displays + windows.
    pub fn refresh() -> AppResult<Self> {
        if !scap::is_supported() {
            return Err(AppError::NotSupported);
        }
        // scap panics here when permission is absent — guard explicitly.
        if !scap::has_permission() {
            return Err(AppError::ScreenPermissionDenied);
        }

        let mut targets = std::panic::catch_unwind(std::panic::AssertUnwindSafe(scap::get_all_targets))
            .map_err(|_| AppError::ScreenPermissionDenied)?;

        // SCShareableContent can report an empty display list while the panel
        // is asleep or in some screen-sharing sessions, even though the main
        // display still exists. Fall back to it so recording stays possible.
        if !targets.iter().any(|t| matches!(t, scap::Target::Display(_))) {
            log::warn!("[capture] no displays enumerated; falling back to main display");
            targets.push(scap::Target::Display(scap::get_main_display()));
        }
        log::info!("[capture] get_all_targets: {} targets", targets.len());

        let main_id = scap::get_main_display().id;

        let mut items = Vec::with_capacity(targets.len());
        for target in targets {
            let (id, kind, title) = match &target {
                scap::Target::Display(d) => (d.id, "display", d.title.clone()),
                scap::Target::Window(w) => (w.id, "window", w.title.clone()),
            };
            let (w, h, scale) = measure_target(&target);
            log::info!("[capture] target {kind}#{id}: {title} -> {w}x{h} @{scale}");
            // Skip zero-size windows (offscreen AV modules and similar system
            // cruft): they can't produce a capture and only clutter the list.
            if kind == "window" && (w == 0 || h == 0) {
                continue;
            }
            items.push((
                CaptureTargetInfo {
                    id,
                    kind: kind.to_string(),
                    title,
                    is_primary: id == main_id && kind == "display",
                    width: w,
                    height: h,
                    scale_factor: scale,
                },
                target,
            ));
        }

        // Displays first (stable ordering), then windows with a real title.
        items.sort_by_key(|(info, _)| {
            (
                info.kind != "display",
                info.title.is_empty(),
                info.title.to_lowercase(),
            )
        });

        Ok(Self { items })
    }

    pub fn list(&self) -> Vec<CaptureTargetInfo> {
        self.items.iter().map(|(info, _)| info.clone()).collect()
    }

    pub fn find(&self, id: u32) -> Option<&scap::Target> {
        self.items
            .iter()
            .find(|(info, _)| info.id == id)
            .map(|(_, t)| t)
    }

    pub fn find_info(&self, id: u32) -> Option<&CaptureTargetInfo> {
        self.items
            .iter()
            .find(|(info, _)| info.id == id)
            .map(|(info, _)| info)
    }

    /// Resolve the user's source selection into a concrete scap configuration.
    pub fn resolve(&self, spec: &SourceSpec) -> AppResult<ResolvedSource> {
        // Validate the region before the target lookup so the user gets the
        // actionable error first.
        let crop = match spec.kind {
            SourceKind::Display | SourceKind::Window => None,
            SourceKind::Region => {
                let region = spec.region.ok_or(AppError::RegionNotSelected)?;
                if !region.is_valid() {
                    return Err(AppError::RegionNotSelected);
                }
                Some(to_scap_area(&region))
            }
        };

        let target = self
            .find(spec.target_id)
            .cloned()
            .ok_or_else(|| AppError::SourceNotFound(spec.target_id.to_string()))?;

        // Guard against a stale id from another tab (e.g. a display id saved
        // while in window mode): recording it would silently capture the
        // wrong thing.
        let info = self
            .find_info(spec.target_id)
            .expect("target was just found by id");
        let expected = match spec.kind {
            SourceKind::Display | SourceKind::Region => "display",
            SourceKind::Window => "window",
        };
        if info.kind != expected {
            return Err(AppError::SourceNotFound(format!(
                "所选源类型不匹配（需要 {expected}，得到 {}），请重新选择",
                info.kind
            )));
        }

        Ok(ResolvedSource { target, crop })
    }
}

pub struct ResolvedSource {
    pub target: scap::Target,
    pub crop: Option<scap::capturer::Area>,
}

impl ResolvedSource {
    /// Build the scap options for a recording.
    pub fn to_options(
        &self,
        fps: u32,
        quality: QualityPreset,
        show_cursor: bool,
        captures_system_audio: bool,
        excluded: Vec<scap::Target>,
    ) -> scap::capturer::Options {
        scap::capturer::Options {
            fps,
            show_cursor,
            show_highlight: false,
            target: Some(self.target.clone()),
            crop_area: self.crop.clone(),
            output_type: scap::frame::FrameType::BGR0,
            output_resolution: quality.to_scap(),
            excluded_targets: if excluded.is_empty() {
                None
            } else {
                Some(excluded)
            },
            captures_audio: captures_system_audio,
            exclude_current_process_audio: captures_system_audio,
        }
    }
}

pub fn to_scap_area(rect: &Rect) -> scap::capturer::Area {
    let r = rect.rounded();
    scap::capturer::Area {
        origin: scap::capturer::Point { x: r.x, y: r.y },
        size: scap::capturer::Size {
            width: r.width,
            height: r.height,
        },
    }
}

/// Whether the platform supports system-audio capture without extra drivers.
pub fn system_audio_supported() -> bool {
    // macOS: ScreenCaptureKit. Windows: WASAPI loopback.
    cfg!(target_os = "macos") || cfg!(target_os = "windows")
}

/// Best-effort geometry for a target. Window dimensions would require a
/// hand-rolled AppKit struct-return call (NSRect is 32 bytes and objc 0.2.7
/// has no Encode support for it — that silently corrupts the return and
/// SIGBUSes), so windows report 0x0 and the UI hides the size for them.
/// Display geometry comes from [`TargetRegistry::fill_display_geometry`].
/// Best-effort geometry for a target. Uses scap's patched, cross-process-safe
/// helpers; windows whose geometry can't be resolved report 0x0 and the UI
/// hides the size for them. Display geometry is filled separately from the
/// windowing system (see [`TargetRegistry::fill_display_geometry`]).
fn measure_target(target: &scap::Target) -> (u32, u32, f64) {
    let scale = scap::get_scale_factor(target);
    let (w, h) = scap::get_target_dimensions(target);
    (w as u32, h as u32, if scale > 0.0 { scale } else { 1.0 })
}

impl TargetRegistry {
    /// Fill display geometry from the windowing system, matched by title.
    /// scap exposes no public dimension helper for displays, and the windowing
    /// system already reports logical sizes.
    pub fn fill_display_geometry(&mut self, monitors: &[tauri::Monitor]) {
        for (info, _) in self.items.iter_mut() {
            if info.kind != "display" {
                continue;
            }
            if let Some(m) = monitors
                .iter()
                .find(|m| m.name().is_some_and(|n| n.as_str() == info.title.as_str()))
            {
                let scale = m.scale_factor();
                let size: tauri::LogicalSize<f64> = m.size().to_logical(scale);
                info.width = size.width as u32;
                info.height = size.height as u32;
                info.scale_factor = scale;
            }
        }
    }

    /// Return the window targets whose ids appear in `window_numbers`, so scap
    /// can exclude our own windows from display capture. Constructing scap
    /// window targets is not possible from outside the crate (the `Window`
    /// type is private), so we reuse the enumerated targets instead.
    pub fn excluded_window_targets(&self, window_numbers: &[u32]) -> Vec<scap::Target> {
        self.items
            .iter()
            .filter_map(|(_, target)| match target {
                scap::Target::Window(w) if window_numbers.contains(&w.id) => Some(target.clone()),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_to_scap_area_rounds() {
        let area = to_scap_area(&Rect::new(10.4, 20.6, 800.5, 600.5));
        assert_eq!(area.origin.x, 10.0);
        assert_eq!(area.origin.y, 21.0);
        // f64::round() rounds halves away from zero
        assert_eq!(area.size.width, 801.0);
        assert_eq!(area.size.height, 601.0);
    }

    #[test]
    fn resolve_region_requires_valid_rect() {
        let registry = TargetRegistry { items: vec![] };
        // A region smaller than the 8px minimum is rejected before the
        // (missing) target is even looked up.
        let spec = SourceSpec {
            kind: SourceKind::Region,
            target_id: 1,
            region: Some(Rect::new(0.0, 0.0, 4.0, 4.0)),
        };
        assert!(matches!(
            registry.resolve(&spec),
            Err(AppError::RegionNotSelected)
        ));
    }
}
