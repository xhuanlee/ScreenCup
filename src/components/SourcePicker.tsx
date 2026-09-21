import { AppWindow, Crop, Monitor } from "lucide-react";
import { motion } from "framer-motion";

import { useT } from "../i18n";
import { useStore, type SourceTab } from "../store";
import type { TargetInfo } from "../lib/api";
import { resolutionLabel, truncate } from "../lib/format";
import Dropdown from "./ui/Dropdown";
import type { DropdownOption } from "./ui/Dropdown";

const TABS: { id: SourceTab; key: "display" | "window" | "region"; icon: typeof Monitor }[] = [
  { id: "display", key: "display", icon: Monitor },
  { id: "window", key: "window", icon: AppWindow },
  { id: "region", key: "region", icon: Crop },
];

export default function SourcePicker() {
  const settings = useStore((s) => s.settings);
  const sources = useStore((s) => s.sources);
  const loading = useStore((s) => s.loadingSources);
  const regionPicking = useStore((s) => s.regionPicking);
  const setTab = useStore((s) => s.setTab);
  const pickTarget = useStore((s) => s.pickTarget);
  const startRegionPick = useStore((s) => s.startRegionPick);
  const t = useT();

  const displays = sources.filter((s) => s.kind === "display");
  const windows = sources.filter((s) => s.kind === "window");

  const toOptions = (list: TargetInfo[]): DropdownOption<number>[] =>
    list.map((src) => ({
      value: src.id,
      label:
        src.kind === "display"
          ? src.is_primary
            ? `${src.title}${t("source.primarySuffix")}`
            : src.title
          : truncate(src.title),
      hint: resolutionLabel(src.width, src.height),
    }));

  const selectedSource = sources.find((s) => s.id === settings.target_id);
  const tab = settings.kind;

  return (
    <section className="mt-1">
      <div className="flex gap-1.5 rounded-2xl border border-line bg-panel/60 p-1.5">
        {TABS.map(({ id, key, icon: Icon }) => {
          const active = tab === id;
          return (
            <button
              key={id}
              type="button"
              onClick={() => void setTab(id)}
              className="relative flex flex-1 items-center justify-center gap-1.5 rounded-xl px-1 py-2 text-[12.5px] font-medium transition-colors"
              style={{ color: active ? "var(--color-fg)" : "var(--color-fg-2)" }}
            >
              {active && (
                <motion.span
                  layoutId="tab-pill"
                  transition={{ type: "spring", stiffness: 480, damping: 36 }}
                  className="absolute inset-0 rounded-xl border border-line-2 bg-panel-2"
                />
              )}
              <Icon size={15} className="relative z-10" />
              <span className="relative z-10">{t(`source.${key}`)}</span>
            </button>
          );
        })}
      </div>

      <div className="mt-3.5">
        {tab === "region" ? (
          <div className="space-y-2.5">
            <Dropdown
              value={settings.target_id}
              options={toOptions(displays)}
              onChange={pickTarget}
              placeholder={t("source.selectDisplay")}
              disabled={loading}
            />
            <RegionRow
              region={settings.region}
              picking={regionPicking}
              onPick={startRegionPick}
            />
          </div>
        ) : (
          <Dropdown
            value={settings.target_id}
            options={toOptions(tab === "display" ? displays : windows)}
            onChange={pickTarget}
            placeholder={tab === "display" ? t("source.selectDisplay") : t("source.selectWindow")}
            disabled={loading}
          />
        )}
      </div>

      <SummaryLine
        source={selectedSource}
        kind={tab}
        fps={settings.fps}
        quality={settings.quality}
      />
    </section>
  );
}

function RegionRow({
  region,
  picking,
  onPick,
}: {
  region: { x: number; y: number; width: number; height: number } | null;
  picking: boolean;
  onPick: () => void;
}) {
  const t = useT();
  if (region) {
    return (
      <div className="flex items-center gap-2 rounded-xl border border-accent/40 bg-accent/10 px-3 py-2.5">
        <Crop size={15} className="shrink-0 text-accent" />
        <span className="flex-1 truncate font-mono text-[12px] tabular-nums text-fg">
          {Math.round(region.width)} × {Math.round(region.height)}
          <span className="text-fg-3"> @ ({Math.round(region.x)}, {Math.round(region.y)})</span>
        </span>
        <button
          type="button"
          onClick={onPick}
          disabled={picking}
          className="shrink-0 rounded-lg px-2 py-1 text-[12px] font-medium text-accent transition-colors hover:bg-accent/15 disabled:opacity-50"
        >
          {t("source.repick")}
        </button>
      </div>
    );
  }

  return (
    <button
      type="button"
      onClick={onPick}
      disabled={picking}
      className="group flex w-full items-center justify-center gap-2 rounded-xl border border-dashed border-line-2 bg-panel-2/40 py-3.5 text-[13px] font-medium text-fg-2 transition-colors hover:border-accent/60 hover:text-fg disabled:opacity-50"
    >
      <Crop size={16} className="transition-transform group-hover:scale-110" />
      {picking ? t("source.picking") : t("source.pickRegion")}
    </button>
  );
}

function SummaryLine({
  source,
  kind,
  fps,
  quality,
}: {
  source: TargetInfo | undefined;
  kind: string;
  fps: number;
  quality: string;
}) {
  const t = useT();
  const qualityLabel: Record<string, string> = {
    original: t("quality.original"),
    p1080: "1080P",
    p720: "720P",
    p480: "480P",
  };
  const bits = [
    source ? truncate(source.title, 22) : t("summary.noSource"),
    source ? resolutionLabel(source.width, source.height) : null,
    kind === "region" ? t("summary.regionMode") : null,
    `${fps} fps`,
    qualityLabel[quality] ?? quality,
  ].filter(Boolean);

  return (
    <div className="mt-3 flex items-center justify-center gap-1.5 text-[11px] text-fg-3">
      {bits.map((b, i) => (
        <span key={i} className="flex items-center gap-1.5">
          {i > 0 && <span className="text-line-2">·</span>}
          {b}
        </span>
      ))}
    </div>
  );
}
