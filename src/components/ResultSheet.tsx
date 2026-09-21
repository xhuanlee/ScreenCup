import { convertFileSrc } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import { FolderOpen, Trash2, X } from "lucide-react";
import { useState } from "react";

import { api, type RecordingResult } from "../lib/api";
import { useStore } from "../store";
import { formatBytes, formatDuration, resolutionLabel } from "../lib/format";

export default function ResultSheet() {
  const result = useStore((s) => s.result);
  const clearResult = useStore((s) => s.clearResult);
  const discardRecording = useStore((s) => s.discardRecording);

  return (
    <AnimatePresence>
      {result && (
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: 16 }}
          transition={{ type: "spring", stiffness: 320, damping: 30 }}
          className="glass absolute inset-x-3 bottom-3 z-30 overflow-hidden rounded-2xl border border-line-2 shadow-soft"
        >
          <Preview result={result} />

          <div className="flex items-center gap-2 px-3 py-2.5">
            <div className="min-w-0 flex-1">
              <div className="truncate text-[12.5px] font-medium text-fg">
                {result.file_name}
              </div>
              <div className="truncate font-mono text-[10.5px] text-fg-3">
                {formatDuration(result.duration_ms)} · {resolutionLabel(result.width, result.height)}
                {result.codec ? ` · ${result.codec.toUpperCase()}` : ""} · {formatBytes(result.size_bytes)}
                {result.audio ? "" : " · 无音频"}
              </div>
            </div>

            <IconButton
              label="在文件夹中显示"
              onClick={() => void api.revealFile(result.path)}
            >
              <FolderOpen size={15} />
            </IconButton>

            <IconButton
              label="删除"
              danger
              onClick={() => void discardRecording()}
            >
              <Trash2 size={15} />
            </IconButton>

            <IconButton label="关闭" onClick={clearResult}>
              <X size={15} />
            </IconButton>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Preview({ result }: { result: RecordingResult }) {
  const [failed, setFailed] = useState(false);

  if (failed) {
    return (
      <div className="grid h-[120px] place-items-center bg-bg-2 text-[12px] text-fg-3">
        预览不可用
      </div>
    );
  }

  return (
    <video
      src={convertFileSrc(result.path)}
      className="h-[120px] w-full bg-black object-cover"
      autoPlay
      loop
      muted
      playsInline
      preload="metadata"
      onError={() => setFailed(true)}
    />
  );
}

function IconButton({
  children,
  label,
  onClick,
  danger,
}: {
  children: React.ReactNode;
  label: string;
  onClick: () => void;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      className={`grid h-8 w-8 shrink-0 place-items-center rounded-lg text-fg-2 transition-colors hover:bg-panel-2 ${
        danger ? "hover:bg-danger/20 hover:text-danger" : "hover:text-fg"
      }`}
    >
      {children}
    </button>
  );
}
