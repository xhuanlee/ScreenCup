import { AppWindow, Crop, Monitor } from "lucide-react";
import { motion } from "framer-motion";

import { useStore, type SourceTab } from "../store";
import type { TargetInfo } from "../lib/api";
import { resolutionLabel, truncate } from "../lib/format";
import Dropdown from "./ui/Dropdown";
import type { DropdownOption } from "./ui/Dropdown";

const TABS: { id: SourceTab; label: string; icon: typeof Monitor }[] = [
  { id: "display", label: "整个屏幕", icon: Monitor },
  { id: "window", label: "应用窗口", icon: AppWindow },
  { id: "region", label: "自定义区域", icon: Crop },
];

export default function SourcePicker() {
  const settings = useStore((s) => s.settings);
  const sources = useStore((s) => s.sources);
  const loading = useStore((s) => s.loadingSources);
  const regionPicking = useStore((s) => s.regionPicking);
  const setTab = useStore((s) => s.setTab);
  const pickTarget = useStore((s) => s.pickTarget);
  const startRegionPick = useStore((s) => s.startRegionPick);

  const displays = sources.filter((s) => s.kind === "display");
  const windows = sources.filter((s) => s.kind === "window");

  const toOptions = (list: TargetInfo[]): DropdownOption<number>[] =>
    list.map((t) => ({
      value: t.id,
      label:
        t.kind === "display"
          ? t.is_primary
            ? `${t.title}（主显示器）`
            : t.title
          : truncate(t.title),
      hint: resolutionLabel(t.width, t.height),
    }));

  const selectedSource = sources.find((s) => s.id === settings.target_id);
  const tab = settings.kind;

  return (
    <section className="mt-1">
      <div className="flex gap-1.5 rounded-2xl border border-line bg-panel/60 p-1.5">
        {TABS.map(({ id, label, icon: Icon }) => {
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
              <span className="relative z-10">{label}</span>
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
              placeholder="选择显示器"
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
            placeholder={tab === "display" ? "选择显示器" : "选择窗口"}
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
          重新框选
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
      {picking ? "请在屏幕上框选…" : "框选录制区域"}
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
  const qualityLabel: Record<string, string> = {
    original: "原画",
    p1080: "1080P",
    p720: "720P",
    p480: "480P",
  };
  const bits = [
    source ? truncate(source.title, 22) : "未选择源",
    source ? resolutionLabel(source.width, source.height) : null,
    kind === "region" ? "自定义区域" : null,
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
