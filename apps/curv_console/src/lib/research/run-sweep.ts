/**
 * WP-X.9 (SWP-3): layout helpers for the `summary.sweep` block of a `param_sweep` run. The Rust job computes every
 * cell (rates, means, medians, suppression); these helpers only order the cells for drawing and find the published one.
 */
import type { ParamSpec, RunSummary, SweepSpec } from "@/lib/api/research";
import { isNumeric, type ProfileDoc, valueOf } from "./registry";
import { defaultSweep, MAX_SWEEP_STEPS, sweepValues } from "./sweep";

export type SweepSummary = NonNullable<RunSummary["sweep"]>;
export type SweepCell = SweepSummary["cells"][number];

const sameValues = (a: readonly number[], b: readonly number[]) => a.length === b.length && a.every((v, i) => v === b[i]);
const uniqueSorted = (values: number[]) => [...new Set(values)].sort((a, b) => a - b);

/** Whether `cell` holds the published profile's values (the marked cell). */
export function isPublishedCell(sweep: SweepSummary, cell: SweepCell): boolean {
  return sweep.published_values != null && sweep.published_values.length > 0 && sameValues(cell.values, sweep.published_values);
}

/** Cells ordered by their values (first axis, then second): the curve and table order. */
export function orderedCells(sweep: SweepSummary): SweepCell[] {
  return [...sweep.cells].sort((a, b) => (a.values[0] ?? 0) - (b.values[0] ?? 0) || (a.values[1] ?? 0) - (b.values[1] ?? 0));
}

export interface SweepGridLayout {
  xs: number[];
  ys: number[];
  /** `x \u0000 y` -> cell. */
  at: Map<string, SweepCell>;
  /** The published cell when its values are not on the grid (listed in the table, not drawn in the grid). */
  offGrid: SweepCell | null;
}

export const gridKey = (x: number, y: number) => `${x}\u0000${y}`;

/** The two-axis grid: axis values and a lookup; the published point off the grid is set apart. */
export function gridLayout(sweep: SweepSummary): SweepGridLayout {
  const pub = sweep.cells.find((c) => isPublishedCell(sweep, c)) ?? null;
  const others = sweep.cells.filter((c) => c !== pub);
  const xs = uniqueSorted(others.map((c) => c.values[0] ?? 0));
  const ys = uniqueSorted(others.map((c) => c.values[1] ?? 0));
  const onGrid = pub != null && xs.includes(pub.values[0] ?? NaN) && ys.includes(pub.values[1] ?? NaN);
  const drawn = pub && !onGrid ? others : sweep.cells;
  return {
    xs: onGrid || !pub ? uniqueSorted(drawn.map((c) => c.values[0] ?? 0)) : xs,
    ys: onGrid || !pub ? uniqueSorted(drawn.map((c) => c.values[1] ?? 0)) : ys,
    at: new Map(drawn.map((c) => [gridKey(c.values[0] ?? 0, c.values[1] ?? 0), c])),
    offGrid: pub && !onGrid ? pub : null,
  };
}

/**
 * The shade step (0 .. `steps - 1`) of `value` within `[min, max]`: a sequential one-hue scale drawn with CSS classes
 * (no inline styles under the CSP). Equal bounds give the middle step.
 */
export function shadeStep(value: number, min: number, max: number, steps: number): number {
  if (!(max > min)) return Math.floor(steps / 2);
  const t = (value - min) / (max - min);
  return Math.min(steps - 1, Math.max(0, Math.floor(t * steps)));
}

// ---- starting a sweep (WP-X.9b, RUI-5, SWP-1, SWP-2): the axis editor's pure rules ------------------------------

/** SWP-1 / SWP-2: a sweep has one or two axes and at most this many cells (the server refuses more: `sweep-too-large`). */
export const MAX_SWEEP_AXES = 2;
export const MAX_SWEEP_CELLS = 625;

/** The metric keys a run can summarise per cell (`PerVideoMetricKey`, validated by the server as `options.metrics[i]`). */
export const METRIC_KEYS = [
  "social_dwell_ratio", "geometric_dwell_ratio", "social_preference_score", "eye_dwell_ratio", "mouth_dwell_ratio",
  "aoi_dwell_ratio", "aoi_dwell_window_ratio", "first_entry_latency_ms", "fixation_count", "mean_fixation_duration_ms",
  "social_smile_intensity_mean", "smile_reactivity_index", "flat_affect_score", "surprise_score_mean", "valid_sample_ratio",
] as const;

/** An axis as typed: the knob and the raw text of the three bounds (so a half-typed number is not lost). */
export interface AxisDraft {
  paramId: string;
  from: string;
  to: string;
  step: string;
}

/** Why an axis is refused, with the numbers the message quotes. */
export type AxisProblem =
  | { reason: "param" }
  | { reason: "level" }
  | { reason: "range" }
  | { reason: "step" }
  | { reason: "bounds"; min: number; max: number }
  | { reason: "whole" }
  | { reason: "fine"; step: number }
  | { reason: "too_many"; steps: number };

export type AxisCheck = { ok: true; values: number[]; steps: number } | { ok: false; problem: AxisProblem };

/**
 * The knobs an axis may pick at `level`: Number and Integer rows, never `capture_only` (only a new capture shows
 * them), and the `metrics`/`frames` stage rows only at level `frames` (they need the research frame log).
 * Owner-gated rows stay in: sweeping one reads data, it does not publish anything.
 */
export function axisKnobs(params: readonly ParamSpec[], level: string): ParamSpec[] {
  return params.filter(
    (p) => isNumeric(p.kind) && p.reprocess !== "capture_only" && (p.reprocess === "score" || level === "frames"),
  );
}

/** A starting axis for a knob: its whole registry range in about twenty steps. */
export function newAxis(spec: ParamSpec): AxisDraft {
  const d = defaultSweep(spec.kind);
  return { paramId: spec.id, from: String(d?.from ?? ""), to: String(d?.to ?? ""), step: String(d?.step ?? "") };
}

/**
 * Checks one axis against its registry row the way the server does (SWP-1): finite `from <= to`, a positive step not
 * finer than the row's, whole numbers for an Integer row, values inside the row's `[min, max]`, at most
 * {@link MAX_SWEEP_STEPS} steps. The values are the grid plus the published one when it lies inside and off the grid.
 */
export function checkAxis(spec: ParamSpec | undefined, draft: AxisDraft, level: string, published: ProfileDoc): AxisCheck {
  if (!spec || !isNumeric(spec.kind) || spec.reprocess === "capture_only") return { ok: false, problem: { reason: "param" } };
  if (spec.reprocess !== "score" && level !== "frames") return { ok: false, problem: { reason: "level" } };
  const num = (t: string) => (t.trim() === "" ? Number.NaN : Number(t));
  const [from, to, step] = [num(draft.from), num(draft.to), num(draft.step)] as [number, number, number];
  if (![from, to].every(Number.isFinite) || from > to) return { ok: false, problem: { reason: "range" } };
  if (!Number.isFinite(step) || !(step > 0)) return { ok: false, problem: { reason: "step" } };
  const kind = spec.kind;
  if (from < kind.min || to > kind.max) return { ok: false, problem: { reason: "bounds", min: kind.min, max: kind.max } };
  if (kind.type === "integer" && ![from, to, step].every(Number.isInteger)) return { ok: false, problem: { reason: "whole" } };
  const finest = kind.type === "number" ? kind.step : 1;
  if (step < finest * (1 - 1e-9)) return { ok: false, problem: { reason: "fine", step: finest } };
  const steps = Math.floor((to - from) / step + 1e-9);
  if (steps > MAX_SWEEP_STEPS) return { ok: false, problem: { reason: "too_many", steps } };
  const pub = valueOf(published, spec);
  const values = sweepValues(kind, from, to, step, typeof pub === "number" ? pub : Number.NaN);
  return values.ok ? { ok: true, values: values.values, steps } : { ok: false, problem: { reason: "range" } };
}

export type SweepProblem = "axes" | "same_knob" | "cells";

export interface SweepCheck {
  axes: AxisCheck[];
  /** Product of the axes' value counts (published value included); `null` while an axis is invalid. */
  cells: number | null;
  /** Whole-sweep problem, else `null`. */
  problem: SweepProblem | null;
  ok: boolean;
}

/** Checks every axis, that no knob repeats, and the cell budget (`cells <= MAX_SWEEP_CELLS`). */
export function checkSweep(params: readonly ParamSpec[], axes: readonly AxisDraft[], level: string, published: ProfileDoc): SweepCheck {
  const checks = axes.map((a) => checkAxis(params.find((p) => p.id === a.paramId), a, level, published));
  if (axes.length < 1 || axes.length > MAX_SWEEP_AXES) return { axes: checks, cells: null, problem: "axes", ok: false };
  const ids = axes.map((a) => a.paramId);
  const cells = checks.every((c) => c.ok) ? checks.reduce((n, c) => n * (c.ok ? c.values.length : 1), 1) : null;
  const problem: SweepProblem | null = new Set(ids).size !== ids.length ? "same_knob" : cells !== null && cells > MAX_SWEEP_CELLS ? "cells" : null;
  return { axes: checks, cells, problem, ok: cells !== null && problem === null };
}

/** The request's `sweep` block: the typed bounds as numbers. */
export function sweepSpec(axes: readonly AxisDraft[]): SweepSpec {
  return { axes: axes.map((a) => ({ param_id: a.paramId, from: Number(a.from), to: Number(a.to), step: Number(a.step) })) };
}

/** Where a server field name (`sweep.axes[1].step`, `options.metrics[0]`) belongs in the dialog. */
export function fieldTarget(name: string): { kind: "axis"; index: number } | { kind: "metrics" } | { kind: "sweep" } | { kind: "other" } {
  const axis = /^sweep\.axes\[(\d+)\]/.exec(name);
  if (axis) return { kind: "axis", index: Number(axis[1]) };
  if (name.startsWith("options.metrics")) return { kind: "metrics" };
  if (name === "sweep" || name.startsWith("sweep.")) return { kind: "sweep" };
  return { kind: "other" };
}
