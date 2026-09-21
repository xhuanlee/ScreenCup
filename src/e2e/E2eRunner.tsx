import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { api, type Settings } from "../lib/api";
import { useStore } from "../store";

type Mode = "plain" | "audio" | "mic" | "window" | "region" | "pause";

/**
 * Headless self-test scenarios. The backend sets `#e2e-<mode>` via
 * SCREENCUT_E2E=1 + SCREENCUT_E2E_MODE=<mode>. Each scenario configures the
 * app, records for a few seconds, stops, and probes the output so the harness
 * can verify the produced file end to end.
 */
export default function E2eRunner({ mode = "plain" }: { mode?: Mode }) {
  const [line, setLine] = useState(`e2e: starting (mode=${mode})`);
  const log = (m: string) => {
    setLine(m);
    void invoke("log_frontend", { message: `e2e: ${m}` }).catch(() => {});
  };

  useEffect(() => {
    void (async () => {
      try {
        await useStore.getState().init();
        log("init done");

        const base: Settings = {
          ...useStore.getState().settings,
          kind: "display",
          target_id: null,
          region: null,
          capture_system_audio: false,
          capture_mic: false,
          mic_device: null,
          hide_main_while_recording: false,
        };

        let settings = base;
        const sources = await api.listSources();

        switch (mode) {
          case "audio":
            settings = { ...base, capture_system_audio: true };
            break;
          case "mic": {
            settings = { ...base, capture_system_audio: true, capture_mic: true };
            const mics = await api.listMicrophones();
            if (mics.length === 0) throw new Error("no microphone available");
            settings.mic_device = mics[0];
            log(`mic: ${mics[0]}`);
            break;
          }
          case "window": {
            // The biggest window is most likely a real app rather than a
            // menu-bar sliver.
            const win = sources
              .filter((s) => s.kind === "window" && s.title)
              .sort((a, b) => b.width * b.height - a.width * a.height)[0];
            if (!win) throw new Error("no capturable window found");
            settings = { ...base, kind: "window", target_id: win.id };
            log(`target: window#${win.id} "${win.title}" ${win.width}x${win.height}`);
            break;
          }
          case "region": {
            const main = sources.find((s) => s.is_primary && s.kind === "display");
            if (!main) throw new Error("no primary display found");
            // a centered 960x540 logical crop of the main display
            const w = 960;
            const h = 540;
            const x = Math.max(0, (main.width - w) / 2);
            const y = Math.max(0, (main.height - h) / 2);
            settings = {
              ...base,
              kind: "region",
              region: { x, y, width: w, height: h },
              target_id: main.id,
            };
            log(`region: ${JSON.stringify(settings.region)}`);
            break;
          }
          case "plain":
          default: {
            const main = sources.find((s) => s.is_primary && s.kind === "display");
            if (!main) throw new Error("no primary display found");
            settings = { ...base, target_id: main.id };
            log(`target: display#${main.id} (${main.width}x${main.height})`);
            break;
          }
        }

        await api.saveSettings(settings);
        log("settings saved");

        await api.startRecording();
        log("recording started");

        if (mode === "pause") {
          await new Promise((r) => setTimeout(r, 2500));
          await api.pauseRecording();
          log("paused");
          await new Promise((r) => setTimeout(r, 2000));
          await api.resumeRecording();
          log("resumed");
          await new Promise((r) => setTimeout(r, 2500));
        } else {
          await new Promise((r) => setTimeout(r, 6000));
        }
        log("stopping…");

        const result = await api.stopRecording();
        log(
          `result: ${result.path} (${Math.round(result.size_bytes / 1024)} KB, ${result.duration_ms} ms, audio=${result.audio})`,
        );
        if (result.warnings.length > 0) log(`warnings: ${result.warnings.join("; ")}`);

        const probe = await invoke<string>("probe_file", { path: result.path });
        log(`probe: ${probe}`);
        log("DONE");
      } catch (e) {
        log(`FAILED: ${String(e)}`);
      }
    })();
  }, [mode]);

  return (
    <div style={{ padding: 24, fontFamily: "monospace", color: "#e5e7eb", background: "#0b0d17", height: "100vh" }}>
      <pre>{line}</pre>
    </div>
  );
}
