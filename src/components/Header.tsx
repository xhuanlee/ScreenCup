import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { FileText, Minus, X } from "lucide-react";
import { motion } from "framer-motion";

import Logo from "./Logo";
import { useStore } from "../store";
import { useI18n, useT, type Lang } from "../i18n";

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
            {t("app.name")}
          </span>
          {version && (
            <span className="font-mono text-[10px] text-fg-3">v{version}</span>
          )}
        </div>
      </div>

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
