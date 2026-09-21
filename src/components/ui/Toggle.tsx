import { motion } from "framer-motion";

interface ToggleProps {
  checked: boolean;
  onChange: () => void;
  disabled?: boolean;
  "aria-label"?: string;
}

export default function Toggle({ checked, onChange, disabled, ...rest }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={onChange}
      className={`relative h-[26px] w-[44px] shrink-0 rounded-full transition-colors duration-200 outline-none ${
        disabled
          ? "bg-line cursor-not-allowed opacity-60"
          : checked
            ? "bg-accent"
            : "bg-line-2 hover:bg-fg-3/40"
      }`}
      {...rest}
    >
      <motion.span
        layout
        transition={{ type: "spring", stiffness: 520, damping: 34 }}
        className="absolute top-[3px] block h-[20px] w-[20px] rounded-full bg-white shadow-[0_2px_6px_rgba(0,0,0,0.45)]"
        style={{ left: checked ? "21px" : "3px" }}
      />
    </button>
  );
}
