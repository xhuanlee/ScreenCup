import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { homeDir } from "@tauri-apps/api/path";

import {
  api,
  type AppInfo,
  type QualityPreset,
  type Rect,
  type RecordingResult,
  type SessionState,
  type Settings,
  type SourceKind,
  type TargetInfo,
} from "./lib/api";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type SourceTab = Extract<SourceKind, "display" | "window" | "region">;

interface UiState {
  booting: boolean;
  loadingSources: boolean;
  starting: boolean;
  stopping: boolean;
  /** Permission wall is up (screen capture not granted). */
  permissionBlocked: boolean;
  regionPicking: boolean;
  error: string | null;
  notice: string | null;
  toasts: ToastEntry[];
}

interface ToastEntry {
  id: number;
  kind: "info" | "error";
  message: string;
}

interface StoreState extends UiState {
  info: AppInfo | null;
  home: string;
  sources: TargetInfo[];
  mics: string[];
  settings: Settings;
  state: SessionState;
  elapsedMs: number;
  result: RecordingResult | null;
  unlisteners: UnlistenFn[];

  init: () => Promise<void>;
  refreshSources: () => Promise<void>;
  refreshMics: () => Promise<void>;
  setTab: (tab: SourceTab) => Promise<void>;
  pickTarget: (id: number) => Promise<void>;
  pickMic: (name: string | null) => Promise<void>;
  startRegionPick: () => Promise<void>;
  toggleSystemAudio: () => Promise<void>;
  toggleMic: () => Promise<void>;
  toggleCursor: () => Promise<void>;
  setQuality: (q: QualityPreset) => Promise<void>;
  setFps: (fps: number) => Promise<void>;
  chooseFolder: () => Promise<void>;
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<void>;
  togglePause: () => Promise<void>;
  discardRecording: () => Promise<void>;
  clearResult: () => void;
  dismissError: () => void;
  pushToast: (kind: ToastEntry["kind"], message: string) => void;
  setElapsed: (ms: number) => void;
  setState: (state: SessionState) => void;
}

const DEFAULT_SETTINGS: Settings = {
  kind: "display",
  target_id: null,
  region: null,
  capture_system_audio: true,
  capture_mic: false,
  mic_device: null,
  show_cursor: true,
  fps: 30,
  quality: "original",
  output_dir: null,
  hide_main_while_recording: true,
};

export const useStore = create<StoreState>((set, get) => ({
  booting: true,
  loadingSources: false,
  starting: false,
  stopping: false,
  permissionBlocked: false,
  regionPicking: false,
  error: null,
  notice: null,
  toasts: [],

  info: null,
  home: "",
  sources: [],
  mics: [],
  settings: DEFAULT_SETTINGS,
  state: "idle",
  elapsedMs: 0,
  result: null,
  unlisteners: [],

  init: async () => {
    invoke("log_frontend", { message: "init: start" }).catch(() => {});
    const [info, settings, state, last, home] = await Promise.all([
      api.getAppInfo(),
      api.getSettings().catch(() => DEFAULT_SETTINGS),
      api.getRecordingState(),
      api.getLastResult(),
      homeDir().catch(() => ""),
    ]);
    invoke("log_frontend", { message: "init: parallel batch done" }).catch(() => {});

    const unlisteners: UnlistenFn[] = [];
    unlisteners.push(
      await listen<number>("screencut://elapsed", (e) => {
        set({ elapsedMs: e.payload });
      }),
    );
    unlisteners.push(
      await listen<{ state: SessionState }>("screencut://state", (e) => {
        set({ state: e.payload.state });
        if (e.payload.state === "idle") {
          set({ elapsedMs: 0 });
        }
      }),
    );
    unlisteners.push(
      await listen<RecordingResult>("screencut://result", (e) => {
        set({ result: e.payload });
      }),
    );
    // The region overlay is a separate webview: it saves the rect server-side,
    // and this event mirrors the outcome into the main window's store so the
    // record button unlocks (null payload = the user cancelled).
    unlisteners.push(
      await listen<Rect | null>("screencut://region-confirmed", (e) => {
        set((s) => ({
          regionPicking: false,
          settings: e.payload
            ? { ...s.settings, region: e.payload }
            : { ...s.settings },
        }));
      }),
    );
    unlisteners.push(
      await listen<string>("screencut://error", (e) => {
        get().pushToast("error", e.payload);
      }),
    );
    unlisteners.push(
      await listen<string>("screencut://hotkey", async (e) => {
        const current = get().state;
        if (e.payload === "toggle") {
          if (current === "recording" || current === "paused") {
            await get().stopRecording();
          } else if (current === "idle") {
            await get().startRecording();
          }
        } else if (e.payload === "pause-toggle") {
          if (current === "recording") {
            await get().togglePause();
          } else if (current === "paused") {
            await get().togglePause();
          }
        }
      }),
    );

    set({
      info,
      settings,
      state,
      result: last,
      home,
      booting: false,
      permissionBlocked: !info.permission_granted,
      unlisteners,
    });

    invoke("log_frontend", { message: "init: listeners ready" }).catch(() => {});

    set({
      info,
      settings,
      state,
      result: last,
      home,
      booting: false,
      permissionBlocked: !info.permission_granted,
      unlisteners,
    });

    invoke("log_frontend", { message: "init: state set" }).catch(() => {});

    if (info.permission_granted) {
      await get().refreshSources();
    }
    await get().refreshMics();
  },

  refreshSources: async () => {
    set({ loadingSources: true });
    try {
      const sources = await api.listSources();
      const settings = { ...get().settings };
      let changed = false;
      if (settings.target_id === null) {
        const main = sources.find((s) => s.is_primary && s.kind === "display");
        if (main) {
          settings.target_id = main.id;
          changed = true;
        }
      }
      set({ sources, settings, loadingSources: false });
      if (changed) {
        await api.saveSettings(settings);
      }
    } catch (e) {
      set({ loadingSources: false });
      get().pushToast("error", errorMessage(e));
    }
  },

  refreshMics: async () => {
    try {
      const mics = await api.listMicrophones();
      set({ mics });
    } catch {
      /* microphones are optional */
    }
  },

  setTab: async (tab) => {
    const { settings, sources } = get();
    // The target id must match the new kind: an id from the display list is
    // meaningless in window mode (and would silently record the wrong source),
    // so auto-select the first source of the new kind.
    const sameKind = sources.find((s) => s.kind === tab && s.id === settings.target_id);
    const firstOfKind = sources.find((s) => s.kind === tab);
    const target_id = sameKind ? settings.target_id : firstOfKind?.id ?? null;
    const next = { ...settings, kind: tab, target_id };
    // Region mode keeps the display target but must not keep a stale region
    // that belonged to another display geometry.
    set({ settings: next });
    await api.saveSettings(next).catch((e) => {
      get().pushToast("error", errorMessage(e));
    });
  },

  pickTarget: async (id) => {
    const settings = { ...get().settings, target_id: id };
    set({ settings });
    await api.saveSettings(settings);
  },

  pickMic: async (name) => {
    const settings = { ...get().settings, mic_device: name };
    set({ settings });
    await api.saveSettings(settings);
  },

  startRegionPick: async () => {
    const { settings } = get();
    if (settings.target_id === null) {
      get().pushToast("error", "请先选择一个显示器");
      return;
    }
    set({ regionPicking: true });
    try {
      await api.openRegionOverlay(settings.target_id);
    } catch (e) {
      set({ regionPicking: false });
      get().pushToast("error", errorMessage(e));
    }
  },

  toggleSystemAudio: async () => {
    const settings = {
      ...get().settings,
      capture_system_audio: !get().settings.capture_system_audio,
    };
    set({ settings });
    await api.saveSettings(settings);
  },

  toggleMic: async () => {
    const on = !get().settings.capture_mic;
    const settings = { ...get().settings, capture_mic: on };
    set({ settings });
    await api.saveSettings(settings);
    if (on) await get().refreshMics();
  },

  toggleCursor: async () => {
    const settings = { ...get().settings, show_cursor: !get().settings.show_cursor };
    set({ settings });
    await api.saveSettings(settings);
  },

  setQuality: async (quality) => {
    const settings = { ...get().settings, quality };
    set({ settings });
    await api.saveSettings(settings);
  },

  setFps: async (fps) => {
    const settings = { ...get().settings, fps };
    set({ settings });
    await api.saveSettings(settings);
  },

  chooseFolder: async () => {
    try {
      const dir = await api.chooseOutputDir();
      if (dir) {
        const settings = { ...get().settings, output_dir: dir };
        set({ settings });
        await api.saveSettings(settings);
      }
    } catch (e) {
      get().pushToast("error", errorMessage(e));
    }
  },

  startRecording: async () => {
    const { state, settings, info } = get();
    if (state === "recording" || state === "paused") return;
    if (settings.kind === "region" && !settings.region) {
      get().pushToast("error", "请先框选录制区域");
      return;
    }
    if (info && !info.has_ffmpeg) {
      get().pushToast("error", "未找到 ffmpeg，无法编码视频");
      return;
    }
    set({ starting: true, error: null });
    try {
      await api.startRecording();
      set({ state: "recording", elapsedMs: 0 });
    } catch (e) {
      get().pushToast("error", errorMessage(e));
    } finally {
      set({ starting: false });
    }
  },

  stopRecording: async () => {
    if (get().stopping) return;
    set({ stopping: true });
    try {
      const result = await api.stopRecording();
      set({ result, state: "idle", elapsedMs: 0 });
      if (result.warnings.length > 0) {
        get().pushToast("info", result.warnings[0]);
      }
    } catch (e) {
      get().pushToast("error", errorMessage(e));
      set({ state: "idle" });
    } finally {
      set({ stopping: false });
    }
  },

  togglePause: async () => {
    const state = get().state;
    try {
      if (state === "recording") {
        await api.pauseRecording();
        set({ state: "paused" });
      } else if (state === "paused") {
        await api.resumeRecording();
        set({ state: "recording" });
      }
    } catch (e) {
      get().pushToast("error", errorMessage(e));
    }
  },

  discardRecording: async () => {
    const result = get().result;
    set({ result: null });
    if (result) {
      try {
        await api.deleteFile(result.path);
        get().pushToast("info", "已丢弃该录制");
      } catch (e) {
        get().pushToast("error", errorMessage(e));
        set({ result });
      }
    }
  },

  clearResult: () => set({ result: null }),
  dismissError: () => set({ error: null }),
  pushToast: (kind, message) => {
    const id = Date.now() + Math.floor(Math.random() * 1000);
    set((s) => ({ toasts: [...s.toasts, { id, kind, message }] }));
    setTimeout(() => {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    }, 4200);
  },
  setElapsed: (ms) => set({ elapsedMs: ms }),
  setState: (state) => set({ state }),
}));

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "未知错误";
}
