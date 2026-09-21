import { getCurrentWindow } from "@tauri-apps/api/window";
import { MonitorUp, RefreshCw, FolderPlus } from "lucide-react";
import { useEffect, useState } from "react";

import { api } from "../lib/api";
import { useStore } from "../store";

export default function PermissionGate() {
  const refreshSources = useStore((s) => s.refreshSources);
  const info = useStore((s) => s.info);
  const [busy, setBusy] = useState(false);
  const [checked, setChecked] = useState(false);

  const apply = async (granted: boolean) => {
    useStore.setState({ permissionBlocked: !granted });
    if (granted) await refreshSources();
  };

  const request = async () => {
    setBusy(true);
    try {
      await api.requestPermission();
      // The system prompt takes the user away; re-check when we get focus back.
      await apply(await api.checkPermission());
    } finally {
      setBusy(false);
    }
  };

  // Re-check permission without (re-)triggering the system prompt — used after
  // the user has already toggled the switch in System Settings.
  const recheck = async () => {
    setBusy(true);
    try {
      await apply(await api.checkPermission());
    } finally {
      setBusy(false);
    }
  };

  // Re-evaluate permission whenever the window regains focus (the user may
  // have toggled the switch in System Settings). The check also falls back to
  // a real capture probe, so a stale preflight answer cannot trap the user on
  // this page after they have granted permission.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    const win = getCurrentWindow();
    win
      .onFocusChanged(({ payload: focused }) => {
        if (!focused) return;
        setChecked(true);
        api.checkPermission().then((granted) => {
          const blocked = !granted;
          if (blocked !== useStore.getState().permissionBlocked) {
            void apply(granted);
          }
        });
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, [refreshSources]);

  // macOS never grants screen recording to an app launched straight from a
  // mounted DMG (it runs from a random sandbox path), so no amount of
  // toggling in System Settings will help until it is installed.
  const needsInstall = info?.needs_install ?? false;

  return (
    <div className="flex flex-col items-center px-6 pt-16 text-center">
      <div className="grid h-20 w-20 place-items-center rounded-3xl border border-line-2 bg-panel-2/60 shadow-soft">
        <MonitorUp size={34} className="text-accent" />
      </div>
      <h2 className="mt-6 text-lg font-semibold text-fg">需要屏幕录制权限</h2>
      {needsInstall ? (
        <p className="mt-2.5 max-w-[300px] text-balance text-[13px] leading-relaxed text-amber-300/90">
          ScreenCut 正在从安装盘或临时位置直接运行，macOS 不会授权给该副本。请将其拖入「应用程序」文件夹后重新打开。
        </p>
      ) : (
        <p className="mt-2.5 max-w-[300px] text-balance text-[13px] leading-relaxed text-fg-2">
          ScreenCut 使用系统原生的屏幕捕获能力来录制您的屏幕、窗口或选定区域。请在系统设置中允许屏幕录制。
        </p>
      )}

      {needsInstall ? (
        <div className="mt-7 flex items-center gap-2 rounded-2xl border border-amber-500/30 bg-amber-500/10 px-4 py-2.5 text-[12px] text-amber-100/80">
          <FolderPlus size={15} className="shrink-0" />
          移动到「应用程序」后重启应用
        </div>
      ) : (
        <button
          type="button"
          onClick={request}
          disabled={busy}
          className="mt-7 flex h-11 w-[210px] items-center justify-center gap-2 rounded-2xl bg-accent text-[14px] font-medium text-white shadow-glow transition-transform active:scale-[0.98] disabled:opacity-60"
        >
          {busy ? (
            <RefreshCw size={16} className="animate-spin" />
          ) : (
            <MonitorUp size={16} />
          )}
          {busy ? "等待授权…" : "授予屏幕录制权限"}
        </button>
      )}

      {!needsInstall && (
        <button
          type="button"
          onClick={recheck}
          disabled={busy}
          className="mt-3 flex items-center gap-1.5 text-[12px] text-fg-3 transition-colors hover:text-fg-2 disabled:opacity-60"
        >
          <RefreshCw size={13} className={busy ? "animate-spin" : ""} />
          已授权？重新检测
        </button>
      )}
      {checked && !needsInstall && (
        <p className="mt-3 text-[11px] text-fg-3">仍未检测到权限，请检查系统设置 → 隐私与安全性 → 屏幕录制</p>
      )}
    </div>
  );
}
