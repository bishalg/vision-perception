/**
 * Typed wrappers over the replay exports of the Rust core (WP-X.20 over WP-X.19 `asd-wasm/src/replay.rs`; ADR-0015
 * V-1..V-4, V-9). Each takes plain values, passes JSON to the module and decodes the `{ok,result}` envelope with a zod
 * schema that mirrors the Rust result type (shape only; every number is the core's). Nothing here detects an event,
 * hits an AOI, windows a trace or smooths a grid: the console only draws what comes back.
 *
 * `tests/replay-wasm.test.ts` runs these against the real module, so a change of the Rust shapes fails a test here.
 */
import { z } from "@/lib/zod";
import type { AsdCore, Envelope } from "./core";

// ---- frames -----------------------------------------------------------------------------------------------------

/**
 * One `GazeFrame` as the core reads it (CON-1): the gaze sample in the Swift wire spelling (`valid`, `x_norm`, ...),
 * frame-level keys camelCase. Loose: the console passes frames to the core untouched and reads only these fields to
 * place the gaze point.
 */
export const gazeFrameSchema = z
  .object({
    gaze: z
      .object({
        valid: z.boolean(),
        x_norm: z.number(),
        y_norm: z.number(),
        video_time_sec: z.number(),
        clip_media_time_sec: z.number().nullable().optional(),
      })
      .loose(),
    activeAOINames: z.array(z.string()).optional(),
    qualityVerdict: z.string(),
    stimulusId: z.string().nullable().optional(),
  })
  .loose();
export type GazeFrame = z.infer<typeof gazeFrameSchema>;

/**
 * The frames of a stored session record (`raw_payload.frames`), or `null` when the record carries none. The live API
 * serves a record without frames unless the device sent them inline; the replay then says so.
 */
export function framesOfRecord(record: unknown): GazeFrame[] | null {
  if (!record || typeof record !== "object") return null;
  const frames = (record as { frames?: unknown }).frames;
  const parsed = z.array(gazeFrameSchema).safeParse(frames);
  return parsed.success && parsed.data.length > 0 ? parsed.data : null;
}

/** The child's age in months as the stored record states it (`ageMonths`), else `null`. */
export function ageMonthsOfRecord(record: unknown): number | null {
  if (!record || typeof record !== "object") return null;
  const v = (record as { ageMonths?: unknown }).ageMonths;
  return typeof v === "number" && Number.isInteger(v) && v >= 0 ? v : null;
}

// ---- catalog clip -----------------------------------------------------------------------------------------------

export const replayAreaSchema = z.object({
  name: z.string(),
  startSec: z.number(),
  endSec: z.number(),
  xMin: z.number(),
  yMin: z.number(),
  xMax: z.number(),
  yMax: z.number(),
  roles: z.array(z.string()),
});
export type ReplayArea = z.infer<typeof replayAreaSchema>;

export const replayClipSchema = z
  .object({
    id: z.string(),
    title: z.string(),
    category: z.string(),
    durationSec: z.number(),
    areas: z.array(replayAreaSchema),
  })
  .loose();
export type ReplayClip = z.infer<typeof replayClipSchema>;

/** The catalog clip `id` of a `StimulusCatalog` body, or `null` (unknown body or clip). */
export function catalogClip(catalog: unknown, id: string): ReplayClip | null {
  const clips = (catalog as { clips?: unknown } | null)?.clips;
  if (!Array.isArray(clips)) return null;
  for (const c of clips) {
    const parsed = replayClipSchema.safeParse(c);
    if (parsed.success && parsed.data.id === id) return parsed.data;
  }
  return null;
}

/**
 * The AOI the published baselines read a clip's dwell against (`aoi_dwell_ratio` expectation's `targetAOI`), else
 * the clip's first area; `null` without areas. A lookup in the documents, no rule of its own.
 */
export function targetAoiOf(baselines: unknown, clip: ReplayClip | null): string | null {
  const video = (baselines as { videos?: Record<string, unknown> } | null)?.videos?.[clip?.id ?? ""] as
    | { segments?: { expectations?: { biomarker?: string; targetAOI?: string | null }[] }[] }
    | undefined;
  for (const seg of video?.segments ?? []) {
    for (const e of seg.expectations ?? []) {
      if (e.biomarker === AOI_DWELL_BIOMARKER && e.targetAOI) return e.targetAOI;
    }
  }
  return clip?.areas[0]?.name ?? null;
}

/** `PerVideoMetricKey::AoiDwellRatio` of the Rust core (the baseline key a `aoiDwell` trace is read against). */
export const AOI_DWELL_BIOMARKER = "aoi_dwell_ratio";

// ---- events -----------------------------------------------------------------------------------------------------

const normPointSchema = z.object({ xNorm: z.number(), yNorm: z.number() });

export const gazeEventSchema = z.discriminatedUnion("kind", [
  z.object({
    kind: z.literal("fixation"),
    startSec: z.number(),
    endSec: z.number(),
    xNorm: z.number(),
    yNorm: z.number(),
    dispersion: z.number(),
  }),
  z.object({
    kind: z.literal("saccade"),
    startSec: z.number(),
    endSec: z.number(),
    from: normPointSchema,
    to: normPointSchema,
    amplitudeNorm: z.number(),
    peakVelocity: z.number(),
  }),
  z.object({ kind: z.literal("blink"), startSec: z.number(), endSec: z.number() }),
  z.object({ kind: z.literal("lost"), startSec: z.number(), endSec: z.number() }),
]);
export type GazeEvent = z.infer<typeof gazeEventSchema>;
export type GazeEventKind = GazeEvent["kind"];

export const clipEventsSchema = z.object({ stimulusId: z.string(), events: z.array(gazeEventSchema) });
export type ClipEvents = z.infer<typeof clipEventsSchema>;

export const replayEventsSchema = z.object({
  params: z
    .object({
      algorithm: z.enum(["ivt", "idt"]),
      ivtVelocityUnitsPerSec: z.number(),
      minFixationDurationSec: z.number(),
      idtDispersionNorm: z.number(),
      fallbackFrameIntervalSec: z.number(),
      blinkMinSec: z.number(),
      blinkMaxSec: z.number(),
    })
    .loose(),
  clips: z.array(clipEventsSchema),
  warnings: z.array(z.string()),
});
export type ReplayEvents = z.infer<typeof replayEventsSchema>;

// ---- ribbon -----------------------------------------------------------------------------------------------------

export const ribbonSegmentSchema = z.object({
  startSec: z.number(),
  endSec: z.number(),
  valid: z.boolean(),
  aoi: z.string().nullable(),
  roles: z.array(z.string()),
  hits: z.array(z.string()),
});
export type RibbonSegment = z.infer<typeof ribbonSegmentSchema>;

export const replayRibbonSchema = z.object({ stimulusId: z.string(), segments: z.array(ribbonSegmentSchema) });
export type ReplayRibbon = z.infer<typeof replayRibbonSchema>;

// ---- trace and bands --------------------------------------------------------------------------------------------

export type TraceMetric =
  | { kind: "aoiDwell"; aoi: string }
  | { kind: "roleDwell"; role: string }
  | { kind: "anyAoi" }
  | { kind: "validRatio" };

const traceMetricSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("aoiDwell"), aoi: z.string() }),
  z.object({ kind: z.literal("roleDwell"), role: z.string() }),
  z.object({ kind: z.literal("anyAoi") }),
  z.object({ kind: z.literal("validRatio") }),
]);

export const tracePointSchema = z.object({
  tSec: z.number(),
  value: z.number().nullable(),
  validRatio: z.number(),
  nFrames: z.number(),
});
export type TracePoint = z.infer<typeof tracePointSchema>;

export const bandThresholdSchema = z.object({ labelKey: z.string(), value: z.number() });
export type BandThreshold = z.infer<typeof bandThresholdSchema>;

export const expectedBandSchema = z.object({
  segmentId: z.string(),
  startSec: z.number(),
  endSec: z.number(),
  metricKey: z.string(),
  direction: z.enum(["higherBetter", "lowerBetter", "band"]),
  bracketId: z.string(),
  low: z.number().nullable(),
  high: z.number().nullable(),
  mean: z.number().nullable(),
  thresholds: z.array(bandThresholdSchema),
  citation: z.string(),
  source: z.string(),
});
export type ExpectedBand = z.infer<typeof expectedBandSchema>;

export const replayTraceSchema = z.object({
  stimulusId: z.string(),
  trace: z.object({
    metric: traceMetricSchema,
    windowSec: z.number(),
    stepSec: z.number(),
    points: z.array(tracePointSchema),
  }),
  bracketId: z.string().nullable(),
  bands: z.array(expectedBandSchema),
});
export type ReplayTrace = z.infer<typeof replayTraceSchema>;

// ---- heatmaps ---------------------------------------------------------------------------------------------------

export const gridSchema = z.object({
  w: z.number().int(),
  h: z.number().int(),
  /** Row-major from the top-left, each in [0, 1] (a difference grid: [-1, 1]). */
  cells: z.array(z.number()),
  nSamples: z.number(),
  nGrids: z.number(),
});
export type Grid = z.infer<typeof gridSchema>;

export const cohortHeatmapSchema = z.object({
  n: z.number(),
  minN: z.number(),
  suppressed: z.boolean(),
  grid: gridSchema.nullable(),
});
export type CohortHeatmap = z.infer<typeof cohortHeatmapSchema>;

// ---- wrappers ---------------------------------------------------------------------------------------------------

function decode<T>(env: Envelope<unknown>, schema: z.ZodType<T>, what: string): Envelope<T> {
  if (!env.ok) return env;
  const r = schema.safeParse(env.result);
  return r.success
    ? { ok: true, result: r.data }
    : { ok: false, error: { kind: "invalidJson", message: `${what}: unexpected result shape (${r.error.issues[0]?.message ?? "?"})` } };
}

const json = (v: unknown) => JSON.stringify(v);

/** Fixations, saccades, blinks and loss of every clip, with the profile's `metrics` parameters. */
export async function replayEvents(core: AsdCore, frames: readonly unknown[], profile: unknown): Promise<Envelope<ReplayEvents>> {
  return decode(await core.call("replayEvents", json(frames), json(profile)), replayEventsSchema, "replayEvents");
}

/** The AOI-hit ribbon of one catalog clip over its frames (`aoi.*` from the profile when given). */
export async function replayRibbon(
  core: AsdCore,
  frames: readonly unknown[],
  clip: ReplayClip,
  profile?: unknown,
): Promise<Envelope<ReplayRibbon>> {
  return decode(
    await core.call("replayRibbon", json(frames), json(clip), profile === undefined ? undefined : json(profile)),
    replayRibbonSchema,
    "replayRibbon",
  );
}

export interface TraceRequest {
  metric: TraceMetric;
  windowSec: number;
  stepSec: number;
  ageMonths?: number | null;
}

/** The windowed metric trace of one clip and, with an age, the published expected bands for the child's bracket. */
export async function replayTrace(
  core: AsdCore,
  frames: readonly unknown[],
  clip: ReplayClip,
  baselines: unknown,
  profile: unknown,
  req: TraceRequest,
): Promise<Envelope<ReplayTrace>> {
  return decode(
    await core.call(
      "replayTrace",
      json(frames),
      json(clip),
      json(baselines),
      json(profile),
      json(req.metric),
      req.windowSec,
      req.stepSec,
      req.ageMonths ?? undefined,
    ),
    replayTraceSchema,
    "replayTrace",
  );
}

/** The expected bands of any baseline expectation of a clip for a child's age (clip cards without a trace). */
export async function expectedBands(
  core: AsdCore,
  baselines: unknown,
  clipId: string,
  ageMonths: number,
  biomarker: string,
  targetAoi?: string | null,
): Promise<Envelope<ExpectedBand[]>> {
  return decode(
    await core.call("expectedBands", json(baselines), clipId, ageMonths, biomarker, targetAoi ?? undefined),
    z.array(expectedBandSchema),
    "expectedBands",
  );
}

export interface HeatmapRequest {
  clipId?: string | null;
  w: number;
  h: number;
  bandwidthNorm: number;
  fromSec?: number | null;
  toSec?: number | null;
}

/** The session gaze density grid (Gaussian KDE in Rust, max 1). */
export async function replayHeatmap(core: AsdCore, frames: readonly unknown[], req: HeatmapRequest): Promise<Envelope<Grid>> {
  return decode(
    await core.call(
      "replayHeatmap",
      json(frames),
      req.clipId ?? undefined,
      req.w,
      req.h,
      req.bandwidthNorm,
      req.fromSec ?? undefined,
      req.toSec ?? undefined,
    ),
    gridSchema,
    "replayHeatmap",
  );
}

/** The cohort mean of session grids, suppressed below `minN` (the core's default without one). */
export async function heatmapCohort(core: AsdCore, grids: readonly Grid[], minN?: number): Promise<Envelope<CohortHeatmap>> {
  return decode(await core.call("heatmapCohort", json(grids), minN), cohortHeatmapSchema, "heatmapCohort");
}

/** `a - b`, clamped to [-1, 1] by the core. */
export async function heatmapDifference(core: AsdCore, a: Grid, b: Grid): Promise<Envelope<Grid>> {
  return decode(await core.call("heatmapDifference", json(a), json(b)), gridSchema, "heatmapDifference");
}
