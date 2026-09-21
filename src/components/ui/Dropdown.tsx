import { AnimatePresence, motion } from "framer-motion";
import { Check, ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { tr } from "../../i18n";

export interface DropdownOption<T extends string | number> {
  value: T;
  label: string;
  hint?: string;
}

interface DropdownProps<T extends string | number> {
  value: T | null | undefined;
  options: DropdownOption<T>[];
  onChange: (value: T) => void;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
}

export default function Dropdown<T extends string | number>({
  value,
  options,
  onChange,
  placeholder,
  disabled,
  className = "",
}: DropdownProps<T>) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const selected = options.find((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    const onPointer = (e: PointerEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={containerRef} className={`relative ${className}`}>
      <button
        type="button"
        disabled={disabled}
        onClick={() => setOpen((v) => !v)}
        className="group flex w-full items-center gap-2 rounded-xl border border-line bg-panel-2/70 px-3 py-2.5 text-left transition-colors hover:border-line-2 disabled:cursor-not-allowed disabled:opacity-50"
      >
        <span className="flex-1 truncate text-sm text-fg">
          {selected?.label ?? placeholder ?? tr("dropdown.placeholder")}
        </span>
        {selected?.hint && (
          <span className="shrink-0 font-mono text-[11px] text-fg-3">{selected.hint}</span>
        )}
        <ChevronDown
          size={15}
          className={`shrink-0 text-fg-3 transition-transform duration-200 ${
            open ? "rotate-180" : ""
          }`}
        />
      </button>

      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: -6, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -6, scale: 0.98 }}
            transition={{ duration: 0.14, ease: [0.16, 1, 0.3, 1] }}
            className="glass absolute left-0 right-0 top-[calc(100%+6px)] z-50 max-h-[260px] overflow-y-auto rounded-2xl border border-line-2 p-1.5 shadow-soft"
          >
            {options.map((option) => {
              const active = option.value === value;
              return (
                <button
                  key={String(option.value)}
                  type="button"
                  onClick={() => {
                    onChange(option.value);
                    setOpen(false);
                  }}
                  className="flex w-full items-center gap-2.5 rounded-xl px-2.5 py-2 text-left transition-colors hover:bg-accent/15"
                >
                  <span className="flex-1 truncate text-sm text-fg group-hover:text-white">
                    {option.label}
                  </span>
                  {option.hint && (
                    <span className="shrink-0 font-mono text-[11px] text-fg-3">
                      {option.hint}
                    </span>
                  )}
                  {active && <Check size={15} className="shrink-0 text-accent" />}
                </button>
              );
            })}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
