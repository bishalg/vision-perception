/**
 * Single-session sweep stepping (RUI-4, SWP-4). Pure: the values to evaluate for one numeric knob. The sweep itself
 * (re-scoring at each value) is the Rust core's work; this only walks a range.
 */
import type { ParamKind } from "@/lib/api/research";
import { clampParam } from "./registry";

/** RUI-4: at most 24 steps from `from` to `to`, so at most 25 values. */
export const MAX_SWEEP_STEPS = 24;
/** Values including the published one when it falls off the grid. */
export const MAX_SWEEP_VALUES = MAX_SWEEP_STEPS + 1;

export type SweepRange = { ok: true; values: number[] } | { ok: false; reason: "range" | "step" | "too_many" };

/**
 * Values `from, from + step, … ≤ to`, each clamped to the row's limits, plus `published` when it lies inside
 * `[from, to]` and is not on the grid (SWP-2). `(to - from) / step` above {@link MAX_SWEEP_STEPS} is refused.
 */
export function sweepValues(kind: ParamKind, from: number, to: number, step: number, published: number): SweepRange {
  if (![from, to, step].every(Number.isFinite) || from > to) return { ok: false, reason: "range" };
  if (!(step > 0)) return { ok: false, reason: "step" };
  const count = Math.floor((to - from) / step + 1e-9);
  if (count > MAX_SWEEP_STEPS) return { ok: false, reason: "too_many" };
  const values: number[] = [];
  for (let i = 0; i <= count; i += 1) {
    const v = clampParam(kind, from + i * step);
    if (v !== null && !values.includes(v)) values.push(v);
  }
  const p = clampParam(kind, published);
  if (p !== null && p >= from && p <= to && !values.includes(p)) values.push(p);
  values.sort((a, b) => a - b);
  return { ok: true, values };
}

/** A starting range for a knob: its whole registry range in about twenty steps (never more than the sweep allows). */
export function defaultSweep(kind: ParamKind): { from: number; to: number; step: number } | null {
  if (kind.type !== "number" && kind.type !== "integer") return null;
  const unit = kind.type === "integer" ? 1 : kind.step;
  const span = kind.max - kind.min;
  const step = Math.max(unit, Math.ceil(span / 20 / unit) * unit);
  return { from: kind.min, to: kind.max, step: Number(step.toPrecision(6)) };
}
