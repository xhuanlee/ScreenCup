import { motion } from "framer-motion";
import { Loader2, Square } from "lucide-react";

import { useStore } from "../store";

export default function RecordButton() {
  const state = useStore((s) => s.state);
  const starting = useStore((s) => s.starting);
  const stopping = useStore((s) => s.stopping);
  const settings = useStore((s) => s.settings);
  const info = useStore((s) => s.info);
  const startRecording = useStore((s) => s.startRecording);
  const stopRecording = useStore((s) => s.stopRecording);

  const recording = state === "recording" || state === "paused";
  const regionMissing = settings.kind === "region" && !settings.region;
  const noFfmpeg = info !== null && !info.has_ffmpeg;
  const busy = starting || stopping || state === "stopping";

  const disabled = busy || regionMissing || noFfmpeg;
  const hint = noFfmpeg
    ? "未检测到 ffmpeg"
    : regionMissing
      ? "请先框选录制区域"
      : recording
        ? "点击停止并保存"
        : "开始录制";

  const onClick = () => {
    if (recording) void stopRecording();
    else void startRecording();
  };

  return (
    <div className="flex flex-col items-center gap-2">
      <motion.button
        type="button"
        onClick={onClick}
        disabled={disabled}
        whileTap={{ scale: 0.94 }}
        className={`relative grid h-[72px] w-[72px] place-items-center rounded-full transition-colors duration-200 outline-none ${
          disabled
            ? "cursor-not-allowed bg-line-2"
            : recording
              ? "bg-panel-2 border border-line-2 hover:border-danger/60"
              : "bg-gradient-to-br from-danger to-danger-2 shadow-[0_8px_28px_rgba(244,63,94,0.45)]"
        }`}
        aria-label={recording ? "停止录制" : "开始录制"}
      >
        {!recording && !disabled && (
          <span
            className="pointer-events-none absolute inset-0 rounded-full"
            style={{ animation: "pulse-ring 2.2s ease-out infinite" }}
          />
        )}
        {busy ? (
          <Loader2 size={26} className="animate-spin text-fg" />
        ) : recording ? (
          <Square size={24} className="fill-danger text-danger" />
        ) : (
          <span className="h-6 w-6 rounded-full bg-white/95" />
        )}
      </motion.button>

      <span className="text-[12px] font-medium text-fg-2">{hint}</span>
      <span className="font-mono text-[10.5px] text-fg-3">
        {info?.platform === "macos" ? "⇧⌘R" : "Ctrl+Shift+R"} 开始 / 停止 ·{" "}
        {info?.platform === "macos" ? "⇧⌘P" : "Ctrl+Shift+P"} 暂停
      </span>
    </div>
  );
}
