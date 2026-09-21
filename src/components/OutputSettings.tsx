import { FolderOpen, MousePointerClick } from "lucide-react";

import { useT } from "../i18n";
import { useStore } from "../store";
import type { QualityPreset } from "../lib/api";
import { shortPath } from "../lib/format";
import Dropdown from "./ui/Dropdown";
import Toggle from "./ui/Toggle";

export default function OutputSettings() {
  const settings = useStore((s) => s.settings);
  const home = useStore((s) => s.home);
  const info = useStore((s) => s.info);
  const setQuality = useStore((s) => s.setQuality);
  const setFps = useStore((s) => s.setFps);
  const toggleCursor = useStore((s) => s.toggleCursor);
  const chooseFolder = useStore((s) => s.chooseFolder);
  const t = useT();

  const QUALITIES: { value: QualityPreset; label: string; hint: string }[] = [
    { value: "original", label: t("quality.original"), hint: t("quality.best") },
    { value: "p1080", label: "1080P", hint: t("quality.smooth") },
    { value: "p720", label: "720P", hint: t("quality.smaller") },
    { value: "p480", label: "480P", hint: t("quality.smallest") },
  ];

  const FPS = [
    { value: 24, label: "24", hint: t("quality.cinematic") },
    { value: 30, label: "30", hint: t("quality.standard") },
    { value: 60, label: "60", hint: t("quality.buttery") },
  ];

  const dir = shortPath(settings.output_dir ?? info?.default_output_dir ?? "", home);

  return (
    <section className="mt-3 rounded-2xl border border-line bg-panel/60 p-4">
      <div className="grid grid-cols-2 gap-3">
        <Field label={t("output.quality")}>
          <Dropdown
            value={settings.quality}
            options={QUALITIES}
            onChange={(q) => void setQuality(q)}
          />
        </Field>
        <Field label={t("output.fps")}>
          <Dropdown value={settings.fps} options={FPS} onChange={(f) => void setFps(f)} />
        </Field>
      </div>

      <button
        type="button"
        onClick={() => void chooseFolder()}
        className="mt-3 flex w-full items-center gap-3 rounded-xl border border-line bg-panel-2/60 px-3 py-2.5 text-left transition-colors hover:border-line-2"
      >
        <FolderOpen size={16} className="shrink-0 text-fg-2" />
        <span className="min-w-0 flex-1">
          <span className="block text-[11px] text-fg-3">{t("output.location")}</span>
          <span className="block truncate text-[12.5px] text-fg-2">
            {dir || t("output.defaultDir")}
          </span>
        </span>
        <span className="shrink-0 text-[12px] font-medium text-accent">{t("output.change")}</span>
      </button>

      <div className="mt-3 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5 text-[12.5px] text-fg-2">
          <MousePointerClick size={15} className="text-fg-3" />
          {t("output.cursor")}
        </div>
        <Toggle
          checked={settings.show_cursor}
          onChange={toggleCursor}
          aria-label={t("output.cursor")}
        />
      </div>
    </section>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="mb-1.5 px-0.5 text-[11px] text-fg-3">{label}</div>
      {children}
    </div>
  );
}
