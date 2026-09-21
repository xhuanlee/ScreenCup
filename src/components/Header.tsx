import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { FileText, Minus, X } from "lucide-react";

import Logo from "./Logo";
import { useStore } from "../store";

export default function Header() {
  const info = useStore((s) => s.info);
  const isMac = info?.platform === "macos";
  const version = info?.version;

  const minimize = () => void getCurrentWindow().minimize();
  const close = () => void getCurrentWindow().close();
  const openLogs = () => {
    if (info?.log_path) void invoke("reveal_file", { path: info.log_path });
  };

  return (
    <header
      data-tauri-drag-region
      className="flex h-[52px] shrink-0 select-none items-center gap-2 px-4"
    >
      <div
        className={`flex flex-1 items-center gap-2.5 ${isMac ? "pl-[72px]" : ""}`}
        data-tauri-drag-region
      >
        <Logo size={24} />
        <div className="flex items-baseline gap-1.5">
          <span className="text-[15px] font-semibold tracking-tight text-fg">
            ScreenCut
          </span>
          {version && (
            <span className="font-mono text-[10px] text-fg-3">v{version}</span>
          )}
        </div>
      </div>

      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={openLogs}
          disabled={!info?.log_path}
          title="打开日志文件"
          className="grid h-8 w-8 place-items-center rounded-lg text-fg-2 transition-colors hover:bg-panel-2 hover:text-fg disabled:opacity-40"
          aria-label="打开日志文件"
        >
          <FileText size={15} />
        </button>
        {!isMac && (
          <>
            <button
              type="button"
              onClick={minimize}
              className="grid h-8 w-8 place-items-center rounded-lg text-fg-2 transition-colors hover:bg-panel-2 hover:text-fg"
              aria-label="最小化"
            >
              <Minus size={15} />
            </button>
            <button
              type="button"
              onClick={close}
              className="grid h-8 w-8 place-items-center rounded-lg text-fg-2 transition-colors hover:bg-danger/20 hover:text-danger"
              aria-label="关闭"
            >
              <X size={15} />
            </button>
          </>
        )}
      </div>
    </header>
  );
}
