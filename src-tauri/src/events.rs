//! Event names emitted to the frontend.

use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub enum AppEvent {
    /// Recording state changed: `idle` | `recording` | `paused` | `stopping`.
    State,
    /// Elapsed active recording time in milliseconds.
    Elapsed,
    /// A recording finished; payload is [`crate::recorder::RecordingResult`].
    Result,
    /// Something went wrong; payload is a human-readable message.
    Error,
    /// Audio input level (0.0 - 1.0), for microphone meters.
    MicLevel,
    /// The user confirmed a region in the overlay; payload is [`crate::geometry::Rect`].
    /// Emitted from the overlay's own webview, which does not share the main
    /// window's store, so the main window learns about the selection this way.
    RegionConfirmed,
}

impl AppEvent {
    pub const fn as_str(self) -> &'static str {
        match self {
            AppEvent::State => "screencut://state",
            AppEvent::Elapsed => "screencut://elapsed",
            AppEvent::Result => "screencut://result",
            AppEvent::Error => "screencut://error",
            AppEvent::MicLevel => "screencut://mic-level",
            AppEvent::RegionConfirmed => "screencut://region-confirmed",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StatePayload {
    pub state: String,
    pub elapsed_ms: u64,
}
