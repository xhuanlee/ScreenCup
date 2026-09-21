use serde::Serialize;

/// Errors that can be surfaced to the frontend.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("找不到 ffmpeg。请确保 ffmpeg 已安装并在系统 PATH 中，或将其放入应用资源目录。")]
    FfmpegNotFound,

    #[error("ffmpeg 退出时返回错误码 {0}")]
    FfmpegFailed(i32),

    #[error("没有屏幕录制权限。请在「系统设置 → 隐私与安全性 → 屏幕录制」中授权 ScreenCut。")]
    ScreenPermissionDenied,

    #[error("没有麦克风权限。请在「系统设置 → 隐私与安全性 → 麦克风」中授权 ScreenCut。")]
    MicPermissionDenied,

    #[error("不支持当前系统的屏幕捕获")]
    NotSupported,

    #[error("录制源未找到：{0}")]
    SourceNotFound(String),

    #[error("尚未选择录制区域")]
    RegionNotSelected,

    #[error("音频设备不可用：{0}")]
    AudioDeviceUnavailable(String),

    #[error("已经在录制中")]
    AlreadyRecording,

    #[error("当前不在录制状态")]
    NotRecording,

    #[error("内部错误：{0}")]
    Internal(String),
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::Internal(format!("窗口或系统操作失败：{e}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Internal(format!("文件操作失败：{e}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(format!("配置解析失败：{e}"))
    }
}

/// cpal's error types carry no useful user-facing message; stringifying them
/// keeps `?` ergonomics in the audio module.
macro_rules! cpal_error {
    ($($t:ident),+ $(,)?) => {
        $(
            impl From<cpal::$t> for AppError {
                fn from(e: cpal::$t) -> Self {
                    AppError::AudioDeviceUnavailable(format!("{e}"))
                }
            }
        )+
    };
}

cpal_error!(DevicesError, DeviceNameError, SupportedStreamConfigsError, DefaultStreamConfigError);

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
