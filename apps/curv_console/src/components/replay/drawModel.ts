/**
 * The replay's draw model (WP-X.20, ADR-0015 V-1): pure functions that turn what the Rust core returned (events, AOI
 * ribbon, trace, bands) plus the frames and the playhead time into draw operations and chart geometry. No event is
 * detected and no AOI is hit-tested here; the canvas and SVG components only paint these ops. Tested without a canvas
 * (`tests/replay-draw.test.ts`).
 */
import type { ExpectedBand, GazeEvent, GazeFrame, ReplayArea, RibbonSegment, TracePoint } from "@/lib/asd/replay";

/** Trail length behind the gaze point (V-1). */
export const TRAIL_SEC = 0.3;
/** Gaze point radius, px. */
export const GAZE_RADIUS_PX = 6;
/** Fixation radius: base + scale x duration / full, capped (V-1: "radius is duration"). */
export const FIXATION_BASE_PX = 4;
export const FIXATION_SCALE_PX = 20;
export const FIXATION_FULL_SEC = 2;
/** How long a finished fixation or saccade stays drawn behind the playhead, seconds. */
export const EVENT_MEMORY_SEC = 1.5;
/** Playback speeds offered (V-1: 0.25x to 2x). */
export const SPEEDS = [0.25, 0.5, 1, 2] as const;
/** The roles that have a colour token (`--role-<role>`); any other role draws as `none`. */
export const ROLE_TOKENS = ["social", "geometric", "eye", "mouth", "orienting"] as const;
export type RoleToken = (typeof ROLE_TOKENS)[number] | "none";

export function roleToken(roles: readonly string[] | null | undefined): RoleToken {
  const first = roles?.[0];
  return (ROLE_TOKENS as readonly string[]).includes(first ?? "") ? (first as RoleToken) : "none";
}

/** One frame of a clip as the canvas places it: clip media time and screen-normalised position. */
export interface Sample {
  t: number;
  x: number;
  y: number;
  valid: boolean;
  /** Session time (`video_time_sec`). */
  sessionSec: number;
}

/** The frames of clip `clipId` as samples ordered by clip time (reads the frame fields; computes nothing). */
export function clipSamples(frames: readonly GazeFrame[], clipId: string): Sample[] {
  return frames
    .filter((f) => f.stimulusId === clipId)
    .map((f) => ({
      t: f.gaze.clip_media_time_sec ?? f.gaze.video_time_sec,
      x: f.gaze.x_norm,
      y: f.gaze.y_norm,
      valid: f.gaze.valid,
      sessionSec: f.gaze.video_time_sec,
    }))
    .sort((a, b) => a.t - b.t);
}

/** Index of the last sample at or before `t` (0 when `t` is before the first). */
export function sampleIndexAt(samples: readonly Sample[], t: number): number {
  let lo = 0;
  let hi = samples.length - 1;
  if (hi < 0) return 0;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (samples[mid]!.t <= t) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** The clip time one frame before (`dir = -1`) or after (`dir = 1`) `t`, clamped to the clip's frames. */
export function stepFrame(samples: readonly Sample[], t: number, dir: 1 | -1): number {
  if (!samples.length) return t;
  const i = sampleIndexAt(samples, t);
  const atOrPast = samples[i]!.t <= t + 1e-9;
  const next = dir === 1 ? (atOrPast ? i + 1 : i) : samples[i]!.t < t - 1e-9 ? i : i - 1;
  return samples[Math.min(samples.length - 1, Math.max(0, next))]!.t;
}

/** The clip's time span: first to last frame (0..0 without frames). */
export function clipSpan(samples: readonly Sample[]): { start: number; end: number } {
  return samples.length ? { start: samples[0]!.t, end: samples[samples.length - 1]!.t } : { start: 0, end: 0 };
}

/** The ribbon segment that contains `t`, else null. */
export function segmentAt(segments: readonly RibbonSegment[], t: number): RibbonSegment | null {
  return segments.find((s) => t >= s.startSec && t < s.endSec) ?? null;
}

export function fixationRadius(durationSec: number): number {
  const k = Math.min(Math.max(durationSec, 0) / FIXATION_FULL_SEC, 1);
  return FIXATION_BASE_PX + FIXATION_SCALE_PX * k;
}

export type DrawOp =
  | { op: "aoi"; x: number; y: number; w: number; h: number; role: RoleToken; label: string; hit: boolean }
  | { op: "saccade"; x1: number; y1: number; x2: number; y2: number; alpha: number }
  | { op: "fixation"; x: number; y: number; r: number; current: boolean; alpha: number }
  | { op: "trail"; x: number; y: number; alpha: number; grey: boolean }
  | { op: "gaze"; x: number; y: number; r: number }
  | { op: "gap"; kind: "blink" | "lost"; x: number; y: number };

export interface DrawInput {
  areas: readonly ReplayArea[];
  events: readonly GazeEvent[];
  samples: readonly Sample[];
  tSec: number;
  width: number;
  height: number;
  /** AOI names hit at `tSec` (from the ribbon), drawn highlighted. */
  hits?: readonly string[];
}

/** The ops of one canvas frame at `tSec`, in paint order (areas, saccades, fixations, trail, gaze point). */
export function drawOps(input: DrawInput): DrawOp[] {
  const { areas, events, samples, tSec: t, width: W, height: H } = input;
  const px = (x: number) => x * W;
  const py = (y: number) => y * H;
  const hits = new Set(input.hits ?? []);
  const ops: DrawOp[] = [];
  for (const a of areas) {
    if (t < a.startSec || t >= a.endSec) continue;
    ops.push({
      op: "aoi",
      x: px(a.xMin),
      y: py(a.yMin),
      w: px(a.xMax) - px(a.xMin),
      h: py(a.yMax) - py(a.yMin),
      role: roleToken(a.roles),
      label: a.name,
      hit: hits.has(a.name),
    });
  }
  const fade = (end: number) => (end >= t ? 1 : Math.max(0, 1 - (t - end) / EVENT_MEMORY_SEC));
  const recent = events.filter((e) => e.startSec <= t && e.endSec >= t - EVENT_MEMORY_SEC);
  for (const e of recent) {
    if (e.kind === "saccade") {
      ops.push({ op: "saccade", x1: px(e.from.xNorm), y1: py(e.from.yNorm), x2: px(e.to.xNorm), y2: py(e.to.yNorm), alpha: fade(e.endSec) });
    }
  }
  for (const e of recent) {
    if (e.kind === "fixation") {
      const current = t >= e.startSec && t <= e.endSec;
      // A running fixation grows with the time already spent in it.
      const r = fixationRadius((current ? t : e.endSec) - e.startSec);
      ops.push({ op: "fixation", x: px(e.xNorm), y: py(e.yNorm), r, current, alpha: fade(e.endSec) });
    }
  }
  if (!samples.length) return ops;
  const gapAt = (time: number) => events.find((e) => (e.kind === "blink" || e.kind === "lost") && time >= e.startSec && time <= e.endSec);
  const i = sampleIndexAt(samples, t);
  let lastValid: Sample | null = null;
  for (let k = i; k >= 0; k--) {
    if (samples[k]!.valid) {
      lastValid = samples[k]!;
      break;
    }
  }
  // Trail: the samples of the last TRAIL_SEC, fading with age; invalid ones are drawn grey at the last valid place.
  let anchor: Sample | null = null;
  const from = sampleIndexAt(samples, t - TRAIL_SEC);
  for (let k = from; k <= i; k++) {
    const s = samples[k]!;
    if (s.t < t - TRAIL_SEC || s.t > t) continue;
    if (s.valid) anchor = s;
    const at = s.valid ? s : anchor;
    if (!at) continue;
    const alpha = Math.max(0, 1 - (t - s.t) / TRAIL_SEC);
    ops.push({ op: "trail", x: px(at.x), y: py(at.y), alpha, grey: !s.valid || Boolean(gapAt(s.t)) });
  }
  const gap = gapAt(t);
  const now = samples[i]!;
  if (gap && (gap.kind === "blink" || gap.kind === "lost")) {
    if (lastValid) ops.push({ op: "gap", kind: gap.kind, x: px(lastValid.x), y: py(lastValid.y) });
  } else if (now.valid && now.t <= t + 1e-9) {
    ops.push({ op: "gaze", x: px(now.x), y: py(now.y), r: GAZE_RADIUS_PX });
  }
  return ops;
}

// ---- trace chart geometry ---------------------------------------------------------------------------------------

export interface ChartBox {
  width: number;
  height: number;
  pad: { l: number; r: number; t: number; b: number };
  /** Clip time axis, seconds. */
  t0: number;
  t1: number;
}

export function xOf(box: ChartBox, t: number): number {
  const span = box.t1 - box.t0 || 1;
  return box.pad.l + ((t - box.t0) / span) * (box.width - box.pad.l - box.pad.r);
}

/** y of a share in [0, 1] (the axis is fixed 0..1). */
export function yOf(box: ChartBox, v: number): number {
  const c = Math.min(1, Math.max(0, v));
  return box.height - box.pad.b - c * (box.height - box.pad.t - box.pad.b);
}

/** SVG path of the trace; a `null` value breaks the line (no valid frame in that window). */
export function tracePath(box: ChartBox, points: readonly TracePoint[]): string {
  let d = "";
  let pen = false;
  for (const p of points) {
    if (p.value === null) {
      pen = false;
      continue;
    }
    d += `${pen ? "L" : "M"}${xOf(box, p.tSec).toFixed(1)},${yOf(box, p.value).toFixed(1)}`;
    pen = true;
  }
  return d;
}

export interface BandRect {
  key: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** The mean line, when the band has one. */
  meanY: number | null;
  band: ExpectedBand;
}

/** The shaded rectangles of the expected bands: an open side runs to the axis end; a band without bounds is skipped. */
export function bandRects(box: ChartBox, bands: readonly ExpectedBand[]): BandRect[] {
  return bands
    .filter((b) => b.low !== null || b.high !== null)
    .map((b) => {
      const x0 = xOf(box, Math.max(b.startSec, box.t0));
      const x1 = xOf(box, Math.min(b.endSec, box.t1));
      const yTop = yOf(box, b.high ?? 1);
      const yBottom = yOf(box, b.low ?? 0);
      return {
        key: `${b.segmentId}-${b.startSec}`,
        x: x0,
        y: yTop,
        w: Math.max(0, x1 - x0),
        h: Math.max(0, yBottom - yTop),
        meanY: b.mean === null ? null : yOf(box, b.mean),
        band: b,
      };
    });
}

/** The point nearest `t` (hover read-out), or null. */
export function nearestPoint(points: readonly TracePoint[], t: number): TracePoint | null {
  let best: TracePoint | null = null;
  for (const p of points) if (!best || Math.abs(p.tSec - t) < Math.abs(best.tSec - t)) best = p;
  return best;
}

/** The share of a clip-card bar: where the recorded value and the band sit on a 0..1 track, in percent. */
export function barGeometry(recorded: number | null, band: ExpectedBand | null): { dot: number | null; from: number; to: number } | null {
  const pct = (v: number) => Math.min(100, Math.max(0, v * 100));
  const dot = recorded === null ? null : pct(recorded);
  if (!band || (band.low === null && band.high === null)) return dot === null ? null : { dot, from: 0, to: 0 };
  return { dot, from: pct(band.low ?? 0), to: pct(band.high ?? 1) };
}
