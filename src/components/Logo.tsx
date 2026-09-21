interface LogoProps {
  size?: number;
  className?: string;
}

/** Inline mark mirroring the app icon: indigo gradient, selection frame, rec dot. */
export default function Logo({ size = 28, className }: LogoProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 1024 1024"
      className={className}
      aria-hidden="true"
    >
      <defs>
        <linearGradient id="sc-logo-grad" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#4F46E5" />
          <stop offset="100%" stopColor="#8B5CF6" />
        </linearGradient>
      </defs>
      <rect x="0" y="0" width="1024" height="1024" rx="226" fill="url(#sc-logo-grad)" />
      <rect
        x="248"
        y="248"
        width="528"
        height="528"
        rx="44"
        fill="none"
        stroke="#FFFFFF"
        strokeOpacity="0.92"
        strokeWidth="22"
      />
      {[
        [248, 248],
        [718, 248],
        [248, 718],
        [718, 718],
      ].map(([x, y]) => (
        <rect
          key={`${x}-${y}`}
          x={x}
          y={y}
          width="58"
          height="58"
          rx="14"
          fill="#FFFFFF"
          stroke="#6366F1"
          strokeWidth="6"
        />
      ))}
      <circle cx="512" cy="512" r="104" fill="#F43F5E" stroke="#FFFFFF" strokeWidth="14" />
    </svg>
  );
}
