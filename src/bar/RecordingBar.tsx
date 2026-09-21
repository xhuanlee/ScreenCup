import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { motion } from "framer-motion";
import { Pause, Play, Square, X } from "lucide-react";
import { useEffect, useState } from "react";

import { api, type SessionState } from "../lib/api";
import { formatDuration } from "../lib/format";

export default function RecordingBar() {
  const [elapsed, setElapsed] = useState(0);
  const [state, setState] = useState<SessionState>("recording");
  const [busy, setBusy] = useState<"stop" | "pause" | null>(null);

  useEffect(() => {
    document.body.classList.add("is-overlay");
    let un1: UnlistenFn | undefined;
    let un2: UnlistenFn | undefined;

    (async () => {
      un1 = await listen<number>("screencut://elapsed", (e) => setElapsed(e.payload));
      un2 = await listen<{ state: SessionState }>("screencut://state", (e) =>
        setState(e.payload.state),
      );
      try {
        const current = await api.getRecordingState();
        setState(current);
      } catch {
        /* window may outlive the session */
      }
    })();

    return () => {
      document.body.classList.remove("is-overlay");
      un1?.();
      un2?.();
    };
  }, []);

  const paused = state === "paused";
  const stopping = state === "stopping" || busy === "stop";

  const togglePause = async () => {
    setBusy("pause");
    try {
      if (paused) await api.resumeRecording();
      else await api.pauseRecording();
    } finally {
      setBusy(null);
    }
  };

  const stop = async (discard: boolean) => {
    setBusy("stop");
    try {
      const result = await api.stopRecording();
      if (discard) {
        await api.deleteFile(result.path);
      }
    } catch {
      /* errors surface on the main window */
    } finally {
      setBusy(null);
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ type: "spring", stiffness: 340, damping: 28 }}
      className="glass flex h-full items-center gap-3 rounded-full border border-line-2 px-4 shadow-soft"
    >
      <div className="relative flex items-center">
        <span
          className="h-2.5 w-2.5 rounded-full"
          style={
            paused
              ? { backgroundColor: "#f59e0b" }
              : { backgroundColor: "#f43f5e", animation: "rec-blink 1.4s ease-in-out infinite" }
          }
        />
      </div>

      <div className="min-w-[64px]">
        <div className="font-mono text-[15px] font-medium tabular-nums text-fg">
          {formatDuration(elapsed)}
        </div>
        <div className="text-[9.5px] uppercase tracking-wider text-fg-3">
          {stopping ? "正在保存" : paused ? "已暂停" : "录制中"}
        </div>
      </div>

      <div className="h-7 w-px bg-line-2" />

      <div className="flex items-center gap-1.5">
        <BarBtn
          label={paused ? "继续" : "暂停"}
          onClick={() => void togglePause()}
          disabled={stopping}
        >
          {paused ? <Play size={14} /> : <Pause size={14} />}
        </BarBtn>
        <BarBtn label="停止并保存" danger onClick={() => void stop(false)} disabled={stopping}>
          <Square size={13} className="fill-current" />
        </BarBtn>
        <BarBtn label="丢弃录制" onClick={() => void stop(true)} disabled={stopping}>
          <X size={14} />
        </BarBtn>
      </div>
    </motion.div>
  );
}

function BarBtn({
  children,
  label,
  onClick,
  disabled,
  danger,
}: {
  children: React.ReactNode;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      className={`grid h-9 w-9 place-items-center rounded-full transition-colors disabled:opacity-50 ${
        danger
          ? "text-danger hover:bg-danger/20"
          : "text-fg-2 hover:bg-panel-2 hover:text-fg"
      }`}
    >
      {children}
    </button>
  );
}
