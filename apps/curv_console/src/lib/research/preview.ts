/**
 * Live-preview helpers (RUI-3), pure: the 150 ms trailing debounce, which changed knobs the score level cannot apply
 * (`param_not_applied`), and the score-level summary of a re-score. No scoring happens here: results come from the
 * Rust core (asd-wasm).
 */
import type { ParamSpec } from "@/lib/api/research";
import type { RescoreResult } from "@/lib/asd/ops";
import { diffParams, type ProfileDoc } from "./registry";

/** RUI-3: the preview re-runs at most once per 150 ms of quiet. */
export const PREVIEW_DEBOUNCE_MS = 150;
/** RUI-3: at most ten sessions are previewed at once. */
export const MAX_PREVIEW_SESSIONS = 10;

export interface Debounced<A extends unknown[]> {
  (...args: A): void;
  cancel(): void;
}

/** Trailing-edge debounce: `fn` runs once, with the last arguments, `ms` after the last call. */
export function debounce<A extends unknown[]>(fn: (...args: A) => void, ms: number = PREVIEW_DEBOUNCE_MS): Debounced<A> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const call = ((...args: A) => {
    if (timer !== undefined) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = undefined;
      fn(...args);
    }, ms);
  }) as Debounced<A>;
  call.cancel = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  return call;
}

export type PreviewLevel = "score" | "frames";

/**
 * Knobs that differ from the published profile but have no effect at `level` (RPX-3): a `frames`/`metrics` knob at the
 * score level, and a `capture_only` knob at any level. The card shows each as `param_not_applied: <id>`.
 */
export function paramsNotApplied(
  registry: readonly ParamSpec[],
  published: ProfileDoc,
  draft: ProfileDoc,
  level: PreviewLevel,
): string[] {
  const changed = new Set(diffParams(registry, published, draft));
  return registry
    .filter((p) => changed.has(p.id))
    .filter((p) => p.reprocess === "capture_only" || (level === "score" && (p.reprocess === "frames" || p.reprocess === "metrics")))
    .map((p) => p.id);
}

export interface ScoreSide {
  composite: number | null;
  flag: "green" | "yellow" | "red" | "unreliable";
  clips: { id: string; title: string; subscore: number; lowConfidence: boolean }[];
  warnings: string[];
}

/** The fields of a core re-score the preview card shows. */
export function summarizeRescore(r: RescoreResult): ScoreSide {
  return {
    composite: r.score.compositeScore,
    flag: r.flag.flag,
    clips: r.score.videos.map((v) => ({ id: v.stimulusId, title: v.title, subscore: v.subscore, lowConfidence: v.lowConfidence })),
    warnings: r.warnings,
  };
}

export interface Trace {
  points: { x: number; y: number }[];
  fixations: { x: number; y: number }[];
}

const isXY = (v: unknown): v is { x: number; y: number } =>
  typeof v === "object" && v !== null && Number.isFinite((v as { x?: unknown }).x) && Number.isFinite((v as { y?: unknown }).y);

/**
 * Reads the `trace` of a frame-level re-process (WP-X.4) when it has the `{points, fixations}` shape the plot draws;
 * anything else is `null` (the card then shows the score-level result only).
 */
export function parseTrace(v: unknown): Trace | null {
  if (typeof v !== "object" || v === null) return null;
  const { points, fixations } = v as { points?: unknown; fixations?: unknown };
  if (!Array.isArray(points) || !points.every(isXY)) return null;
  return { points: points.map(({ x, y }) => ({ x, y })), fixations: Array.isArray(fixations) ? fixations.filter(isXY).map(({ x, y }) => ({ x, y })) : [] };
}
