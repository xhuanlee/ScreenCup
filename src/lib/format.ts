export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "00:00";
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) {
    return `${pad(h)}:${pad(m)}:${pad(s)}`;
  }
  return `${pad(m)}:${pad(s)}`;
}

function pad(n: number): string {
  return n.toString().padStart(2, "0");
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const value = bytes / Math.pow(1024, i);
  const digits = i === 0 ? 0 : value < 10 ? 2 : 1;
  return `${value.toFixed(digits)} ${units[i]}`;
}

export function resolutionLabel(w: number, h: number): string {
  if (!w || !h) return "未知尺寸";
  return `${w} × ${h}`;
}

/** Shorten a window title for dropdown rows. */
export function truncate(text: string, max = 34): string {
  const trimmed = text.trim();
  if (trimmed.length <= max) return trimmed;
  return `${trimmed.slice(0, max - 1)}…`;
}

export function shortPath(dir: string | null | undefined, home: string): string {
  if (!dir) return "";
  if (home && (dir === home || dir.startsWith(`${home}/`) || dir.startsWith(`${home}\\`))) {
    return `~${dir.slice(home.length)}`;
  }
  return dir;
}
