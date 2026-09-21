import { AnimatePresence, motion } from "framer-motion";
import { AlertCircle, Info } from "lucide-react";

import { useStore } from "../store";

export default function Toasts() {
  const toasts = useStore((s) => s.toasts);

  return (
    <div className="pointer-events-none absolute inset-x-0 top-14 z-40 flex flex-col items-center gap-2 px-6">
      <AnimatePresence>
        {toasts.map((t) => (
          <motion.div
            key={t.id}
            initial={{ opacity: 0, y: -10, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -8, scale: 0.96 }}
            transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
            className="glass pointer-events-auto flex max-w-full items-center gap-2.5 rounded-xl border border-line-2 px-3.5 py-2.5 shadow-soft"
          >
            {t.kind === "error" ? (
              <AlertCircle size={15} className="shrink-0 text-danger" />
            ) : (
              <Info size={15} className="shrink-0 text-accent" />
            )}
            <span className="text-[12.5px] leading-relaxed text-fg">{t.message}</span>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
