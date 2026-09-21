import { AnimatePresence, motion } from "framer-motion";
import { Mic, MicOff, Volume2, VolumeX } from "lucide-react";

import { useStore } from "../store";
import Dropdown from "./ui/Dropdown";
import Toggle from "./ui/Toggle";
import { truncate } from "../lib/format";

export default function AudioControls() {
  const info = useStore((s) => s.info);
  const settings = useStore((s) => s.settings);
  const mics = useStore((s) => s.mics);
  const toggleSystemAudio = useStore((s) => s.toggleSystemAudio);
  const toggleMic = useStore((s) => s.toggleMic);
  const pickMic = useStore((s) => s.pickMic);

  const systemSupported = info?.system_audio_supported ?? false;

  return (
    <section className="mt-3 rounded-2xl border border-line bg-panel/60 p-1.5">
      <Row
        icon={
          settings.capture_system_audio ? (
            <Volume2 size={17} className="text-accent" />
          ) : (
            <VolumeX size={17} className="text-fg-3" />
          )
        }
        title="系统音频"
        subtitle={systemSupported ? "录制电脑播放的声音" : "当前系统不支持"}
      >
        <Toggle
          checked={settings.capture_system_audio && systemSupported}
          onChange={toggleSystemAudio}
          disabled={!systemSupported}
          aria-label="录制系统音频"
        />
      </Row>

      <div className="my-1 h-px bg-line" />

      <Row
        icon={
          settings.capture_mic ? (
            <Mic size={17} className="text-accent" />
          ) : (
            <MicOff size={17} className="text-fg-3" />
          )
        }
        title="麦克风"
        subtitle={settings.capture_mic ? "录制您的旁白" : "不录制麦克风"}
      >
        <Toggle
          checked={settings.capture_mic}
          onChange={toggleMic}
          aria-label="录制麦克风"
        />
      </Row>

      <AnimatePresence initial={false}>
        {settings.capture_mic && (
          <motion.div
            key="mic-picker"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
            className="overflow-hidden"
          >
            <div className="px-2.5 pb-2 pt-1">
              {mics.length > 0 ? (
                <Dropdown
                  value={settings.mic_device}
                  options={mics.map((name) => ({
                    value: name,
                    label: truncate(name, 30),
                  }))}
                  onChange={(name) => void pickMic(name)}
                  placeholder="默认设备"
                />
              ) : (
                <p className="rounded-xl bg-panel-2/50 px-3 py-2.5 text-center text-[12px] text-fg-3">
                  未检测到麦克风设备，请检查连接与隐私设置
                </p>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </section>
  );
}

function Row({
  icon,
  title,
  subtitle,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center gap-3 px-2.5 py-2.5">
      <div className="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-panel-2/70">
        {icon}
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-[13.5px] font-medium text-fg">{title}</div>
        <div className="truncate text-[11.5px] text-fg-3">{subtitle}</div>
      </div>
      {children}
    </div>
  );
}
