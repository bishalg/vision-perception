export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  // UTC and fixed locale so server and client render identically (no hydration drift).
  return d.toISOString().slice(0, 16).replace("T", " ") + " UTC";
}

export function formatDuration(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.round(sec % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function formatNumber(value: number | null | undefined, digits = 1): string {
  return value == null ? "-" : value.toFixed(digits);
}

export function formatPercent(value: number | null | undefined): string {
  return value == null ? "-" : `${Math.round(value * 100)}%`;
}
