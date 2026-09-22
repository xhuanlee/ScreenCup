import type { Translations } from "./zh";

/**
 * English strings. Typed against the Chinese dictionary so a missing or
 * extra key is a compile error.
 */
export const en: Translations = {
  app: {
    name: "ScreenCut",
  },

  header: {
    openLogs: "Open log file",
    minimize: "Minimize",
    close: "Close",
    language: "Language",
    languageZh: "中文",
    languageEn: "English",
    dragHint: "Hold to move window",
  },

  source: {
    display: "Full screen",
    window: "App window",
    region: "Custom region",
    primarySuffix: " (main)",
    selectDisplay: "Select a display",
    selectWindow: "Select a window",
    repick: "Reselect",
    pickRegion: "Select recording region",
    picking: "Drag on the screen to select…",
  },

  summary: {
    noSource: "No source selected",
    regionMode: "Custom region",
  },

  quality: {
    original: "Original",
    best: "Highest",
    smooth: "Smooth",
    smaller: "Smaller",
    smallest: "Smallest",
    cinematic: "Cinematic",
    standard: "Standard",
    buttery: "Buttery",
  },

  audio: {
    systemTitle: "System audio",
    systemSubtitleOn: "Record sound played by the computer",
    systemUnsupported: "Not supported on this system",
    micTitle: "Microphone",
    micSubtitleOn: "Record your narration",
    micSubtitleOff: "Microphone off",
    micDefault: "Default device",
    noMics: "No microphone found — check the connection and privacy settings",
  },

  output: {
    quality: "Quality",
    fps: "Frame rate",
    location: "Save to",
    defaultDir: "Default video folder",
    change: "Change",
    cursor: "Record the mouse cursor",
  },

  record: {
    ffmpegMissing: "ffmpeg not detected",
    regionMissing: "Select the recording region first",
    stopHint: "Click to stop and save",
    startHint: "Start recording",
    startAria: "Start recording",
    stopAria: "Stop recording",
    hotkeyStartStop: "Start / Stop",
    hotkeyPause: "Pause",
  },

  perm: {
    title: "Screen recording permission needed",
    needsInstallBody:
      "ScreenCut is running straight from the disk image or a temporary location, and macOS will not authorise that copy. Drag it into the Applications folder and reopen it.",
    body: "ScreenCut uses the system’s native screen capture to record your screen, windows, or a selected region. Please allow screen recording in System Settings.",
    installHint: "Move to Applications, then restart",
    grant: "Grant screen recording permission",
    waiting: "Waiting for permission…",
    recheck: "Already granted? Re-check",
    stillBlocked:
      "Permission still not detected — check System Settings → Privacy & Security → Screen Recording",
  },

  result: {
    noAudio: " · no audio",
    reveal: "Reveal in folder",
    delete: "Delete",
    close: "Close",
    previewUnavailable: "Preview unavailable",
  },

  ffmpeg: {
    prefix: "ScreenCut could not find ",
    first: " on your system, so the video cannot be encoded. Install ",
    middle: " and make sure it is on your PATH, or set the ",
    suffix: " environment variable.",
  },

  region: {
    preparing: "Preparing frame…",
    hintActive: "Drag to move · drag a corner to resize · ←→↑↓ nudge · Enter confirm · Esc cancel",
    hintIdle: "Drag to select · ←→↑↓ nudge · Enter confirm · Esc cancel",
    cancel: "Cancel",
    confirm: "Confirm region",
  },

  bar: {
    saving: "Saving",
    paused: "Paused",
    recording: "Recording",
    resume: "Resume",
    pause: "Pause",
    stop: "Stop and save",
    discard: "Discard",
  },

  toast: {
    selectDisplay: "Select a display first",
    regionFirst: "Select the recording region first",
    noFfmpeg: "ffmpeg not found — cannot encode the video",
    discarded: "Recording discarded",
    unknownError: "Unknown error",
    unknownSize: "Unknown size",
  },

  dropdown: {
    placeholder: "Select…",
  },
};
