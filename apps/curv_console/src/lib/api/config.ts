/**
 * Baselines, scoring profile and config bundles (WP-C.5; backend WP-B.4 `routes/config.rs`): zod schemas for the
 * `NormativeBaselineSet` and `ScoringProfile` document bodies and the `ConfigBundleSummary` DTO of
 * `docs/spec/schemas/asd/api-v1.json`, the `BundlesApi` interface and its live HTTP implementation. Documents of
 * these kinds use the generic config-document API of `./catalog` (`listConfigDocuments` etc.).
 *
 * The schemas check shape only. Value rules (profile ranges, baseline coverage) are the Rust core's: the console
 * runs them through asd-wasm (`@/lib/asd/ops`) and the backend runs the same code on create and approve.
 */
import { z } from "@/lib/zod";
import type { LiveRequest } from "./clinical";

const optStr = z.string().nullable().optional();
const optNum = z.number().nullable().optional();

// ---- NormativeBaselineSet body (asd-core `types::baselines`) ----------------------------------------------------

export const NORMATIVE_DIRECTIONS = ["higherBetter", "lowerBetter", "band"] as const;
export const normativeDirectionSchema = z.enum(NORMATIVE_DIRECTIONS);
export type NormativeDirection = z.infer<typeof normativeDirectionSchema>;

/** Key of the fallback range in `byAge` (asd-core `baselines::DEFAULT_BRACKET_ID`). */
export const DEFAULT_BRACKET_ID = "default";

export const normativeRangeSchema = z.object({ expected: optNum, acceptableLow: optNum, acceptableHigh: optNum });
export type NormativeRange = z.infer<typeof normativeRangeSchema>;

export const normativeExpectationSchema = z.object({
  biomarker: z.string(),
  direction: normativeDirectionSchema,
  targetAOI: optStr,
  /** Keyed by age-bracket id or `"default"`. */
  byAge: z.record(z.string(), normativeRangeSchema),
  citation: z.string(),
  /** Provenance: `literature`, `interpolated` or `own:<norm run>` (free text in the contract). */
  source: z.string(),
  note: optStr,
});
export type NormativeExpectation = z.infer<typeof normativeExpectationSchema>;

export const normativeSegmentSchema = z.object({
  id: z.string(),
  description: z.string(),
  startSec: z.number(),
  endSec: z.number(),
  expectations: z.array(normativeExpectationSchema),
});
export type NormativeSegment = z.infer<typeof normativeSegmentSchema>;

export const normativeVideoSchema = z.object({
  title: z.string(),
  category: z.string(),
  weight: z.number(),
  segments: z.array(normativeSegmentSchema),
});
export type NormativeVideo = z.infer<typeof normativeVideoSchema>;

export const ageBracketSchema = z.object({
  id: z.string(),
  minMonths: z.number().int(),
  maxMonths: z.number().int(),
  /** Profile `ageBands[]` only (DSP-2, 1.3.0): the `ageMonths` a session in the band carries. */
  representativeMonths: z.number().int().optional(),
});
export type AgeBracket = z.infer<typeof ageBracketSchema>;

export const normativeBaselineSetSchema = z.object({
  schemaVersion: z.string(),
  status: z.string(),
  provenance: optStr,
  ageBrackets: z.array(ageBracketSchema),
  /** Keyed by stimulus (clip) id. */
  videos: z.record(z.string(), normativeVideoSchema),
});
export type NormativeBaselineSet = z.infer<typeof normativeBaselineSetSchema>;

// ---- ScoringProfile body (asd-contract `config::ScoringProfile`) -------------------------------------------------

export const flagThresholdsSchema = z.object({
  greenFloor: z.number(),
  yellowFloor: z.number(),
  lowConfidenceShare: z.number(),
  atypicalShareForRed: z.number(),
});
export type FlagThresholds = z.infer<typeof flagThresholdsSchema>;

export const qualityGateParamsSchema = z.object({
  minDistanceCm: z.number(),
  maxDistanceCm: z.number(),
  maxYawRad: z.number(),
  maxPitchRad: z.number(),
  minConfidence: z.number(),
  blinkThreshold: z.number(),
});
export type QualityGateParams = z.infer<typeof qualityGateParamsSchema>;

export const scoringProfileSchema = z.object({
  schemaVersion: z.string(),
  flag: flagThresholdsSchema,
  /** Not applied by the core scorer yet (it uses a fixed 0.60, see the asd-wasm `contractInfo`). */
  lowConfidenceValidRatio: z.number(),
  qualityGate: qualityGateParamsSchema,
  /** Gaze-robustness thresholds (SCH-3); absent in a 1.0.0 profile. Edited as JSON; Rust checks it on decode. */
  trackingQuality: z.record(z.string(), z.unknown()).optional(),
  // Profile 1.2.0 (PRM-7, WP-X.0): absent before 1.2.0 (the Rust decoder fills today's constants). Opaque here; the
  // registry form (WP-X.5) edits them and the Rust registry validates them.
  metrics: z.record(z.string(), z.unknown()).optional(),
  aoi: z.record(z.string(), z.unknown()).optional(),
  ageBands: z.array(ageBracketSchema).optional(),
  norms: z.record(z.string(), z.unknown()).optional(),
  review: z.record(z.string(), z.unknown()).optional(),
  // Profile 1.3.0 (DSP-1, WP-X.0c): result-screen bands and session verdict ratios; absent before 1.3.0. Opaque here
  // like the 1.2.0 sections; the Rust registry validates them.
  display: z.record(z.string(), z.unknown()).optional(),
  sessionQuality: z.record(z.string(), z.unknown()).optional(),
});
export type ScoringProfile = z.infer<typeof scoringProfileSchema>;

// ---- bundles ----------------------------------------------------------------------------------------------------

export const configBundleSummarySchema = z.object({
  id: z.string(),
  bundle_version: z.string(),
  catalog_version: z.string(),
  baselines_version: z.string(),
  scoring_profile_version: z.string(),
  key_id: z.string(),
  published_at: z.string(),
});
export type ConfigBundleSummary = z.infer<typeof configBundleSummarySchema>;

export const configBundlePageSchema = z.object({
  items: z.array(configBundleSummarySchema),
  next_cursor: z.string().nullable().optional(),
  total: z.number().int().min(0).nullable().optional(),
});
export type ConfigBundlePage = z.infer<typeof configBundlePageSchema>;

/** `POST /v1/config/bundles` body (backend `ComposeBundleRequest`, unknown members refused). */
export interface ComposeBundleRequest {
  /** Superadmin only: the org to compose for; omitted = the global default. An org user may only name their org. */
  org_id?: string;
  feature_flags?: Record<string, boolean>;
}

export interface BundlesApi {
  /** Newest first: the caller's org bundles plus the global defaults (superadmin: all). */
  listConfigBundles(query?: { cursor?: string; limit?: number }): Promise<ConfigBundlePage>;
  /**
   * Composes the three *published* documents into a signed bundle (owner/admin for their org, superadmin for any
   * org or the global default). 409 when a kind has no published document, 503 `bundle-signing-unavailable`.
   */
  composeConfigBundle(request: ComposeBundleRequest): Promise<ConfigBundleSummary>;
}

export function createLiveBundles(request: LiveRequest, buildQuery: (q: object) => string): BundlesApi {
  return {
    listConfigBundles: (q) => request(`/config/bundles${buildQuery(q ?? {})}`, configBundlePageSchema),
    composeConfigBundle: (body) => request(`/config/bundles`, configBundleSummarySchema, { method: "POST", body }),
  };
}
