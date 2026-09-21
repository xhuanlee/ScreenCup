import { FolderOpen, MousePointerClick } from "lucide-react";

import { useStore } from "../store";
import type { QualityPreset } from "../lib/api";
import { shortPath } from "../lib/format";
import Dropdown from "./ui/Dropdown";
import Toggle from "./ui/Toggle";

const QUALITIES: { value: QualityPreset; label: string; hint: string }[] = [
  { value: "original", label: "原画", hint: "最高" },
  { value: "p1080", label: "1080P", hint: "流畅" },
  { value: "p720", label: "720P", hint: "较小" },
  { value: "p480", label: "480P", hint: "最小" },
];

const FPS = [
  { value: 24, label: "24", hint: "电影" },
  { value: 30, label: "30", hint: "标准" },
  { value: 60, label: "60", hint: "顺滑" },
];

export default function OutputSettings() {
  const settings = useStore((s) => s.settings);
  const home = useStore((s) => s.home);
  const info = useStore((s) => s.info);
  const setQuality = useStore((s) => s.setQuality);
  const setFps = useStore((s) => s.setFps);
  const toggleCursor = useStore((s) => s.toggleCursor);
  const chooseFolder = useStore((s) => s.chooseFolder);

  const dir = shortPath(settings.output_dir ?? info?.default_output_dir ?? "", home);

  return (
    <section className="mt-3 rounded-2xl border border-line bg-panel/60 p-4">
      <div className="grid grid-cols-2 gap-3">
        <Field label="画质">
          <Dropdown
            value={settings.quality}
            options={QUALITIES}
            onChange={(q) => void setQuality(q)}
          />
        </Field>
        <Field label="帧率">
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
          <span className="block text-[11px] text-fg-3">保存位置</span>
          <span className="block truncate text-[12.5px] text-fg-2">{dir || "默认视频文件夹"}</span>
        </span>
        <span className="shrink-0 text-[12px] font-medium text-accent">更改</span>
      </button>

      <div className="mt-3 flex items-center justify-between gap-3">
        <div className="flex items-center gap-2.5 text-[12.5px] text-fg-2">
          <MousePointerClick size={15} className="text-fg-3" />
          录制鼠标光标
        </div>
        <Toggle checked={settings.show_cursor} onChange={toggleCursor} aria-label="录制鼠标光标" />
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
