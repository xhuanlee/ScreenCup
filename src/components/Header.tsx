import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { FileText, Minus, X } from "lucide-react";
import { motion } from "framer-motion";
import { useRef, useState } from "react";

import Logo from "./Logo";
import { useStore } from "../store";
import { useI18n, useT, type Lang } from "../i18n";

/** Hold the top bar this long before the window starts following the cursor. */
const DRAG_HOLD_MS = 450;
/** Ignore presses that turn into scrolls/clicks before the hold completes. */
const DRAG_MOVE_TOLERANCE = 6;

export default function Header() {
  const info = useStore((s) => s.info);
  const isMac = info?.platform === "macos";
  const version = info?.version;
  const t = useT();
  const lang = useI18n((s) => s.lang);
  const setLang = useI18n((s) => s.setLang);

  const minimize = () => void getCurrentWindow().minimize();
  const close = () => void getCurrentWindow().close();
  const openLogs = () => {
    if (info?.log_path) void invoke("reveal_file", { path: info.log_path });
  };
  const toggleLang = (next: Lang) => void setLang(next);

  const holdTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pressOrigin = useRef<{ x: number; y: number } | null>(null);
  const [arming, setArming] = useState(false);

  const cancelHold = () => {
    if (holdTimer.current !== null) {
      clearTimeout(holdTimer.current);
      holdTimer.current = null;
    }
    pressOrigin.current = null;
    setArming(false);
  };

  // Tauri's built-in `data-tauri-drag-region` starts a drag the instant the
  // mouse goes down, which steals clicks from the buttons and title area.
  // Long-press instead: start dragging only after a deliberate hold, and bail
  // out if the pointer wanders off first. `startDragging` synthesizes a
  // LeftMouseDown event when called outside a real one, so a timer call works.
  const onPointerDown = (e: React.PointerEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    // Buttons manage their own pointer events; a press on them must not arm.
    if (e.target instanceof HTMLElement && e.target.closest("button")) return;
    pressOrigin.current = { x: e.clientX, y: e.clientY };
    setArming(true);
    holdTimer.current = setTimeout(() => {
      holdTimer.current = null;
      setArming(false);
      void getCurrentWindow().startDragging();
    }, DRAG_HOLD_MS);
  };

  const onPointerMove = (e: React.PointerEvent<HTMLElement>) => {
    const origin = pressOrigin.current;
    if (!origin) return;
    if (
      Math.abs(e.clientX - origin.x) > DRAG_MOVE_TOLERANCE ||
      Math.abs(e.clientY - origin.y) > DRAG_MOVE_TOLERANCE
    ) {
      cancelHold();
    }
  };

  return (
    <header
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={cancelHold}
      onPointerLeave={cancelHold}
      className={`relative flex h-[52px] shrink-0 select-none items-center gap-2 px-4 transition-colors ${
        arming ? "bg-panel-2/60" : ""
      }`}
    >
      <div
        className={`flex flex-1 items-center gap-2.5 ${isMac ? "pl-[72px]" : ""}`}
      >
        <Logo size={24} />
        <div className="flex items-baseline gap-1.5">
          <span className="text-[15px] font-semibold tracking-tight text-fg">
            {t("app.name")}
          </span>
          {version && (
            <span className="font-mono text-[10px] text-fg-3">v{version}</span>
          )}
        </div>
      </div>

      {/* Hold-to-drag progress ring: tells the user the press registered and
          roughly how much longer to hold. */}
      {arming && (
        <motion.span
          key="drag-hold-hint"
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0 }}
          className="pointer-events-none absolute left-1/2 top-full -translate-x-1/2 -translate-y-1 rounded-full border border-line-2 bg-panel-2 px-2.5 py-1 text-[10px] text-fg-3 shadow-soft"
        >
          {t("header.dragHint")}
        </motion.span>
      )}

      <div className="flex items-center gap-1">
        <LanguageToggle lang={lang} onSelect={toggleLang} t={t} />
        <button
          type="button"
          onClick={openLogs}
          disabled={!info?.log_path}
          title={t("header.openLogs")}
          className="grid h-8 w-8 place-items-center rounded-lg text-fg-2 transition-colors hover:bg-panel-2 hover:text-fg disabled:opacity-40"
          aria-label={t("header.openLogs")}
        >
          <FileText size={15} />
        </button>
        {!isMac && (
          <>
            <button
              type="button"
              onClick={minimize}
              className="grid h-8 w-8 place-items-center rounded-lg text-fg-2 transition-colors hover:bg-panel-2 hover:text-fg"
              aria-label={t("header.minimize")}
            >
              <Minus size={15} />
            </button>
            <button
              type="button"
              onClick={close}
              className="grid h-8 w-8 place-items-center rounded-lg text-fg-2 transition-colors hover:bg-danger/20 hover:text-danger"
              aria-label={t("header.close")}
            >
              <X size={15} />
            </button>
          </>
        )}
      </div>
    </header>
  );
}

/** A compact 中 / EN pill that flips the whole UI's language. */
function LanguageToggle({
  lang,
  onSelect,
  t,
}: {
  lang: Lang;
  onSelect: (lang: Lang) => void;
  t: ReturnType<typeof useT>;
}) {
  const langs: Lang[] = ["zh", "en"];
  return (
    <div
      className="mr-0.5 flex items-center rounded-lg border border-line bg-panel-2/50 p-0.5"
      role="group"
      aria-label={t("header.language")}
    >
      {langs.map((l) => {
        const active = l === lang;
        return (
          <button
            key={l}
            type="button"
            onClick={() => onSelect(l)}
            aria-pressed={active}
            disabled={active}
            className="relative grid h-6 w-9 place-items-center rounded-md text-[10.5px] font-medium transition-colors disabled:opacity-100"
            style={{ color: active ? "var(--color-fg)" : "var(--color-fg-3)" }}
          >
            {active && (
              <motion.span
                layoutId="lang-pill"
                transition={{ type: "spring", stiffness: 480, damping: 34 }}
                className="absolute inset-0 rounded-md border border-line-2 bg-panel-2"
              />
            )}
            <span className="relative z-10">{l === "zh" ? "中" : "EN"}</span>
          </button>
        );
      })}
    </div>
  );
}
