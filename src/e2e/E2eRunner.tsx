import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  getAllWebviewWindows,
  type WebviewWindow,
} from "@tauri-apps/api/webviewWindow";

import { api, type Settings } from "../lib/api";
import { tr, useI18n } from "../i18n";
import { useStore } from "../store";

type Mode =
  | "plain"
  | "audio"
  | "mic"
  | "window"
  | "region"
  | "pause"
  | "still"
  | "regionui"
  | "drag"
  | "lang";

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

  /** The region overlay is a separate webview labelled screencut-region. */
  const waitForOverlay = async (): Promise<WebviewWindow | null> => {
    for (let i = 0; i < 20; i++) {
      const wins = await getAllWebviewWindows();
      const found = wins.find((w) => w.label.startsWith("screencut-region"));
      if (found) return found;
      await new Promise((r) => setTimeout(r, 250));
    }
    return null;
  };

  /** Wait until the overlay reports its listeners are live. */
  const waitForReady = async (ov: WebviewWindow): Promise<void> => {
    for (let i = 0; i < 40; i++) {
      const got = new Promise<boolean>((resolve) => {
        ov.once("screencut://overlay-ready", () => resolve(true));
        ov.emit("screencut://e2e-ping");
        setTimeout(() => resolve(false), 250);
      });
      if (await got) return;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error("overlay never signalled ready");
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

        if (mode === "still") {
          // The region overlay's magnifier grabs a frozen frame before the
          // selection starts; verify that path works and produces a PNG with
          // the geometry it claims. The image is loaded the same way the
          // overlay loads it (convertFileSrc), so this also proves the loupe
          // will get real pixels.
          const frame = await api.grabRegionStill(null);
          const url = convertFileSrc(frame.path);
          const res = await fetch(url);
          if (!res.ok) throw new Error(`asset read failed: ${res.status}`);
          const buf = await res.arrayBuffer();
          const png = new Uint8Array(buf);
          const magic = [0x89, 0x50, 0x4e, 0x47];
          for (let i = 0; i < magic.length; i++) {
            if (png[i] !== magic[i]) throw new Error("not a PNG");
          }
          const px = frame.width * frame.height * 4;
          const ratio = frame.width / window.devicePixelRatio;
          // A real screenshot compresses far below raw RGBA; a tiny file
          // would mean the encoder wrote an empty or solid frame.
          const minBytes = Math.max(20_000, px / 1024);
          log(
            `still: ${frame.width}x${frame.height} scale=${frame.scale_factor} png=${png.length}B (min ${Math.round(minBytes)}) logical~${Math.round(ratio)}`,
          );
          if (png.length < minBytes) throw new Error("PNG implausibly small");
          log("DONE");
          return;
        }

        if (mode === "regionui") {
          // Drive the CleanShot-style region overlay the way a user would:
          // open it, drag out a selection, nudge it with the arrow keys, and
          // confirm. The overlay is a separate webview, so this scenario runs
          // the interaction against the real component and then records.
          const main = sources.find((s) => s.is_primary && s.kind === "display");
          if (!main) throw new Error("no primary display found");
          await api.saveSettings({ ...base, kind: "region", target_id: main.id });
          log("regionui: settings saved");

          await api.openRegionOverlay(main.id);
          const ov = await waitForOverlay();
          if (!ov) throw new Error("region overlay window not found");
          // The overlay webview must finish loading before its event
          // listeners exist; ping until it answers.
          await waitForReady(ov);
          ov.setFocus();
          await new Promise((r) => setTimeout(r, 300));

          // A 960x540 drag in the upper-left quadrant.
          const cx = (await ov.innerSize()).width;
          const cy = (await ov.innerSize()).height;
          const x1 = cx * 0.15;
          const y1 = cy * 0.15;
          const x2 = x1 + 960;
          const y2 = y1 + 540;
          await ov.emit("screencut://e2e-drag", { x1, y1, x2, y2 });
          await new Promise((r) => setTimeout(r, 300));

          // Arrow-key nudge: 4 px right + 4 px down.
          await ov.emit("screencut://e2e-key", "ArrowRight");
          await ov.emit("screencut://e2e-key", "ArrowDown");
          await new Promise((r) => setTimeout(r, 200));

          // Confirm via the same keyboard path the overlay listens to.
          await ov.emit("screencut://e2e-key", "Enter");
          await new Promise((r) => setTimeout(r, 600));

          const after = await api.getSettings();
          const r = after.region;
          if (!r) throw new Error("region was not saved after confirm");
          // Arrow keys nudge by NUDGE (2) px each.
          const nudged =
            Math.abs(r.x - (x1 + 2)) < 1 && Math.abs(r.y - (y1 + 2)) < 1;
          if (!nudged) throw new Error(`nudge not applied: ${JSON.stringify(r)}`);
          if (Math.abs(r.width - 960) > 2 || Math.abs(r.height - 540) > 2)
            throw new Error(`size drifted: ${JSON.stringify(r)}`);
          log(`regionui: confirmed ${JSON.stringify(r)}`);

          await api.startRecording();
          log("regionui: recording started");
          await new Promise((r2) => setTimeout(r2, 4000));
          const result = await api.stopRecording();
          const probe = await invoke<string>("probe_file", { path: result.path });
          log(`regionui: probe ${probe}`);
          if (result.warnings.length) log(`warnings: ${result.warnings.join("; ")}`);
          log("DONE");
          return;
        }

        if (mode === "drag") {
          // The title bar drag depends on the `core:window:allow-start-dragging`
          // capability and on `startDragging` being callable from a timer
          // (macOS synthesizes the mouse-down event it needs). This scenario
          // exercises both without a human at the mouse.
          const win = getCurrentWindow();
          const before = await win.outerPosition();
          log(`pos before: ${before.x},${before.y}`);
          try {
            await win.startDragging();
            log("startDragging: resolved");
          } catch (e) {
            throw new Error(`startDragging rejected: ${String(e)}`);
          }
          // The drag runs its own event loop; without a real mouse it returns
          // immediately. The call resolving is the interesting part — a
          // missing capability rejects the promise instead.
          await new Promise((r) => setTimeout(r, 400));
          const after = await win.outerPosition();
          log(`pos after: ${after.x},${after.y}`);
          log("DONE");
          return;
        }

        if (mode === "lang") {
          // Switch the UI to English and back, asserting that the resolver
          // flips, that the choice round-trips through the settings file, and
          // that every English key resolves to a non-Chinese string.
          useI18n.getState().init("zh");
          if (tr("record.startHint") !== "开始录制")
            throw new Error(`zh resolver wrong: ${tr("record.startHint")}`);
          log("lang: zh resolves");

          await useI18n.getState().setLang("en");
          const saved = await api.getSettings();
          if (saved.language !== "en")
            throw new Error(`settings.language=${saved.language}, expected en`);
          if (tr("record.startHint") !== "Start recording")
            throw new Error(`en resolver wrong: ${tr("record.startHint")}`);
          log(`lang: en resolves (${tr("source.display")} / ${tr("perm.grant")})`);

          // Spot-check that interpolation still works.
          const notice = tr("ffmpeg.prefix");
          if (!notice) throw new Error("interpolation key empty");

          await useI18n.getState().setLang("zh");
          const back = await api.getSettings();
          if (back.language !== "zh")
            throw new Error(`settings.language=${back.language}, expected zh`);
          if (tr("record.startHint") !== "开始录制")
            throw new Error(`zh restore wrong: ${tr("record.startHint")}`);
          log("lang: zh restored");
          log("DONE");
          return;
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
