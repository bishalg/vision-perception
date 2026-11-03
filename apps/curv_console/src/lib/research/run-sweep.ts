/**
 * WP-X.9 (SWP-3): layout helpers for the `summary.sweep` block of a `param_sweep` run. The Rust job computes every
 * cell (rates, means, medians, suppression); these helpers only order the cells for drawing and find the published one.
 */
import type { RunSummary } from "@/lib/api/research";

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
