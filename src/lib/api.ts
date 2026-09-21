import { invoke } from "@tauri-apps/api/core";

export type SourceKind = "display" | "window" | "region";
export type QualityPreset = "original" | "p1080" | "p720" | "p480";
export type SessionState = "idle" | "recording" | "paused" | "stopping";

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface StillFrame {
  path: string;
  width: number;
  height: number;
  scale_factor: number;
}

export interface AppInfo {
  version: string;
  platform: string;
  arch: string;
  ffmpeg_path: string | null;
  has_ffmpeg: boolean;
  encoder: string | null;
  encoder_hardware: boolean;
  permission_granted: boolean;
  system_audio_supported: boolean;
  default_output_dir: string;
  log_path: string | null;
  needs_install: boolean;
}

export interface TargetInfo {
  id: number;
  kind: string;
  title: string;
  is_primary: boolean;
  width: number;
  height: number;
  scale_factor: number;
}

export interface Settings {
  kind: SourceKind;
  target_id: number | null;
  region: Rect | null;
  capture_system_audio: boolean;
  capture_mic: boolean;
  mic_device: string | null;
  show_cursor: boolean;
  fps: number;
  quality: QualityPreset;
  output_dir: string | null;
  hide_main_while_recording: boolean;
}

export interface RecordingResult {
  path: string;
  file_name: string;
  size_bytes: number;
  duration_ms: number;
  width: number;
  height: number;
  codec: string | null;
  frames: number;
  audio: boolean;
  warnings: string[];
}

export const api = {
  getAppInfo: () => invoke<AppInfo>("get_app_info"),
  listSources: () => invoke<TargetInfo[]>("list_sources"),
  listMicrophones: () => invoke<string[]>("list_microphones"),
  checkPermission: () => invoke<boolean>("check_permission"),
  requestPermission: () => invoke<boolean>("request_permission"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) =>
    invoke<void>("save_settings", { settings }),
  openRegionOverlay: (targetId: number) =>
    invoke<void>("open_region_overlay", { targetId }),
  grabRegionStill: (targetId: number | null) =>
    invoke<StillFrame>("grab_region_still", { targetId }),
  confirmRegion: (rect: Rect) => invoke<void>("confirm_region", { rect }),
  cancelRegion: () => invoke<void>("cancel_region"),
  startRecording: () => invoke<void>("start_recording"),
  stopRecording: () => invoke<RecordingResult>("stop_recording"),
  pauseRecording: () => invoke<void>("pause_recording"),
  resumeRecording: () => invoke<void>("resume_recording"),
  getRecordingState: () => invoke<SessionState>("get_recording_state"),
  getLastResult: () => invoke<RecordingResult | null>("get_last_result"),
  chooseOutputDir: () => invoke<string | null>("choose_output_dir"),
  revealFile: (path: string) => invoke<void>("reveal_file", { path }),
  deleteFile: (path: string) => invoke<void>("delete_file", { path }),
};
