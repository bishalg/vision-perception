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

/** Time since `iso` (queue SLA age): "5m", "2h 30m", "3d 4h". */
export function formatAge(iso: string, now: number = Date.now()): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "-";
  const min = Math.max(0, Math.floor((now - t) / 60_000));
  if (min < 60) return `${min}m`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h}h ${min % 60}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}
