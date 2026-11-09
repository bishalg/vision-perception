/**
 * Trace table and CSV (ADR-0015 R-15: every chart has a table and a CSV of its plotted points) and the band label
 * lookup. Pure; `tests/replay-draw.test.ts` covers them.
 */
import type { TracePoint } from "@/lib/asd/replay";
import { DOMAIN, type DomainLocale } from "@/messages/domain";

/** CSV header: machine column names (stable across locales, like the dataset exports). */
export const TRACE_CSV_HEADER = ["clip_id", "metric", "t_sec", "value", "valid_ratio", "n_frames"] as const;

function cell(v: string | number | null): string {
  if (v === null) return "";
  const s = String(v);
  return /[",\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/** RFC 4180 CSV of the plotted points (CRLF line ends, a trailing line end; an empty window's value is blank). */
export function traceCsv(clipId: string, metric: string, points: readonly TracePoint[]): string {
  const rows = [TRACE_CSV_HEADER.join(",")];
  for (const p of points) rows.push([clipId, metric, p.tSec, p.value, p.validRatio, p.nFrames].map(cell).join(","));
  return rows.join("\r\n") + "\r\n";
}

/** A file name for the CSV: the clip id with anything outside `[A-Za-z0-9_-]` replaced. */
export function traceCsvName(clipId: string): string {
  return `replay-trace-${clipId.replace(/[^A-Za-z0-9_-]/g, "_")}.csv`;
}

/**
 * The text of a band line label key from the core (`replay.band.expected`, ...), through the domain catalogue; an
 * unknown key is shown raw in ‹ › (never throws), like `enumLabel`.
 */
export function bandLabel(locale: DomainLocale, labelKey: string): string {
  const table: Record<string, string | undefined> = DOMAIN[locale];
  return table[labelKey] ?? `‹${labelKey}›`;
}
