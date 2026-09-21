import { AnimatePresence, motion } from "framer-motion";
import { Mic, MicOff, Volume2, VolumeX } from "lucide-react";

import { useT } from "../i18n";
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
  const t = useT();

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
        title={t("audio.systemTitle")}
        subtitle={
          systemSupported
            ? t("audio.systemSubtitleOn")
            : t("audio.systemUnsupported")
        }
      >
        <Toggle
          checked={settings.capture_system_audio && systemSupported}
          onChange={toggleSystemAudio}
          disabled={!systemSupported}
          aria-label={t("audio.systemTitle")}
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
        title={t("audio.micTitle")}
        subtitle={
          settings.capture_mic
            ? t("audio.micSubtitleOn")
            : t("audio.micSubtitleOff")
        }
      >
        <Toggle
          checked={settings.capture_mic}
          onChange={toggleMic}
          aria-label={t("audio.micTitle")}
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
                  placeholder={t("audio.micDefault")}
                />
              ) : (
                <p className="rounded-xl bg-panel-2/50 px-3 py-2.5 text-center text-[12px] text-fg-3">
                  {t("audio.noMics")}
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
