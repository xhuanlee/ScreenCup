use std::path::PathBuf;
use std::sync::RwLock;

use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

use crate::capture::TargetRegistry;
use crate::recorder::{RecordingResult, RecordingSession};
use crate::settings::Settings;

pub struct AppState {
    targets: RwLock<TargetRegistry>,
    settings: RwLock<Settings>,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    session: Mutex<Option<RecordingSession>>,
    last_result: RwLock<Option<RecordingResult>>,
}

impl AppState {
    pub fn new(data_dir: PathBuf, cache_dir: PathBuf) -> Self {
        let settings = Settings::load(&data_dir);
        Self {
            targets: RwLock::new(TargetRegistry::empty()),
            settings: RwLock::new(settings),
            data_dir,
            cache_dir,
            session: Mutex::new(None),
            last_result: RwLock::new(None),
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn set_settings(&self, settings: Settings) {
        *self.settings.write().unwrap() = settings;
    }

    pub fn replace_targets(&self, registry: TargetRegistry) {
        *self.targets.write().unwrap() = registry;
    }

    pub fn session(&self) -> parking_lot::MutexGuard<'_, Option<RecordingSession>> {
        self.session.lock()
    }

    pub fn set_last_result(&self, result: RecordingResult) {
        *self.last_result.write().unwrap() = Some(result);
    }

    pub fn last_result(&self) -> Option<RecordingResult> {
        self.last_result.read().unwrap().clone()
    }
}

/// Run a closure with the current target registry.
pub fn with_targets<T>(app: &AppHandle, f: impl FnOnce(&TargetRegistry) -> T) -> T {
    let state = app.state::<AppState>();
    let reg = state.targets.read().unwrap();
    f(&reg)
}
