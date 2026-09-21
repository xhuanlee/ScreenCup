import { AlertTriangle } from "lucide-react";
import { useEffect } from "react";

import AudioControls from "./components/AudioControls";
import Header from "./components/Header";
import OutputSettings from "./components/OutputSettings";
import PermissionGate from "./components/PermissionGate";
import RecordButton from "./components/RecordButton";
import ResultSheet from "./components/ResultSheet";
import SourcePicker from "./components/SourcePicker";
import Toasts from "./components/Toasts";
import { useStore } from "./store";
import Logo from "./components/Logo";

export default function App() {
  const init = useStore((s) => s.init);
  const booting = useStore((s) => s.booting);
  const permissionBlocked = useStore((s) => s.permissionBlocked);
  const info = useStore((s) => s.info);

  useEffect(() => {
    void init();
  }, [init]);

  if (booting) {
    return (
      <div className="flex h-full flex-col bg-bg">
        <Header />
        <div className="flex flex-1 flex-col items-center justify-center gap-4">
          <Logo size={56} />
          <div className="h-1 w-28 animate-pulse rounded-full bg-line-2" />
        </div>
      </div>
    );
  }

  const ffmpegMissing = info !== null && !info.has_ffmpeg;

  return (
    <div className="flex h-full flex-col bg-bg">
      <Header />
      <div className="relative flex-1 overflow-y-auto px-4 pb-3">
        {permissionBlocked ? (
          <PermissionGate />
        ) : (
          <>
            <SourcePicker />
            <AudioControls />
            <OutputSettings />
            {ffmpegMissing && <FfmpegNotice />}
          </>
        )}
      </div>

      <footer className="shrink-0 border-t border-line/70 bg-bg/80 px-4 pb-5 pt-4">
        <RecordButton />
      </footer>

      <ResultSheet />
      <Toasts />
    </div>
  );
}

function FfmpegNotice() {
  return (
    <div className="mt-3 flex items-start gap-2.5 rounded-2xl border border-amber-500/30 bg-amber-500/10 p-3">
      <AlertTriangle size={16} className="mt-0.5 shrink-0 text-amber-400" />
      <div className="text-[12px] leading-relaxed text-amber-100/80">
        未在系统中找到 <span className="font-mono">ffmpeg</span>，无法完成视频编码。请安装
        <span className="font-mono"> ffmpeg</span> 并确保其在 PATH 中，或设置环境变量
        <span className="font-mono"> SCREENCUT_FFMPEG</span>。
      </div>
    </div>
  );
}
