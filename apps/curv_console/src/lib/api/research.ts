/**
 * Researcher toolkit (ADR-0012, phase 8; WP-X.0 step 0): zod schemas for the `asd_contract::research` and
 * `asd_contract::params` DTOs of `docs/spec/schemas/asd/api-v1.json`, the `ResearchApi` interface and its live HTTP
 * implementation. `research.contract.test.ts` guards the drift. The console computes nothing here: profiles are
 * opaque documents handed to the Rust core (asd-wasm), distributions arrive suppressed (PRV-1) and are drawn as given.
 *
 * Routes answer `501 not-implemented` until WP-X.2 (experiments, cohorts, effective config) and WP-X.3 (runs) land;
 * the mock (`mock/research.ts`) covers every state meanwhile (RUI-7).
 */
import { z } from "@/lib/zod";
import { configDocumentSchema, type ConfigDocument } from "./catalog";
import type { LiveRequest } from "./clinical";

const str = z.string().nullable().optional();
const count = z.number().int().min(0);
const optCount = count.nullable().optional();
const optNum = z.number().nullable().optional();
const pair = z.tuple([z.number(), z.number()]);
/** A whole `ScoringProfile` document (camelCase), opaque here: validated and evaluated by the Rust core. */
const profileSchema = z.record(z.string(), z.unknown());

// ---- parameter registry (PRM-1, param-registry-v1.json) --------------------------------------------------------

export const PARAM_STAGES = ["capture", "preprocess", "fixation", "metrics", "scoring", "flag", "norms", "display"] as const;
export const REPROCESS_LEVELS = ["score", "metrics", "frames", "capture_only"] as const;
export const PARAM_UNITS = [
  "none", "ratio", "percent", "seconds", "millis", "cm", "mm", "deg", "deg_per_sec", "rad", "hz", "norm_units",
  "norm_units_per_sec", "months", "count",
] as const;
const unitSchema = z.enum(PARAM_UNITS);

export const paramKindSchema = z.discriminatedUnion("type", [
  z.object({ type: z.literal("number"), min: z.number(), max: z.number(), step: z.number(), unit: unitSchema }),
  z.object({ type: z.literal("integer"), min: z.number().int(), max: z.number().int(), unit: unitSchema }),
  z.object({ type: z.literal("bool") }),
  z.object({ type: z.literal("enum"), values: z.array(z.string()) }),
  z.object({ type: z.literal("point_list"), minPoints: count, maxPoints: count }),
  z.object({
    type: z.literal("age_bands"),
    minBands: count,
    maxBands: count,
    minMonths: z.number().int(),
    maxMonths: z.number().int(),
  }),
]);
export type ParamKind = z.infer<typeof paramKindSchema>;

/** One registry row. Labels are message keys (`param.<id>.label` / `.help`), resolved in `messages/research.ts`. */
export const paramSpecSchema = z.object({
  id: z.string(),
  document: z.literal("scoring_profile"),
  kind: paramKindSchema,
  default: z.unknown(),
  stage: z.enum(PARAM_STAGES),
  reprocess: z.enum(REPROCESS_LEVELS),
  labelKey: z.string(),
  helpKey: z.string(),
  since: z.string(),
  ownerGate: str,
  /** WP-X.19b: `["replay"]` on a knob only the replay views read; absent when the knob feeds the results. */
  affects: z.array(z.enum(["replay"])).optional(),
});
export type ParamSpec = z.infer<typeof paramSpecSchema>;

export const paramRegistrySchema = z.object({ registryVersion: z.string(), params: z.array(paramSpecSchema) });
export type ParamRegistry = z.infer<typeof paramRegistrySchema>;

// ---- experiments (EXP-1, EXP-2, GOV-2) --------------------------------------------------------------------------

export const EXPERIMENT_STATUSES = ["draft", "archived", "promoted"] as const;
export const experimentStatusSchema = z.enum(EXPERIMENT_STATUSES);
export type ExperimentStatus = z.infer<typeof experimentStatusSchema>;

export const experimentSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  name: z.string(),
  description: str,
  status: experimentStatusSchema,
  base_profile_version: z.string(),
  profile: profileSchema,
  profile_hash: z.string(),
  baselines_version: str,
  promoted_config_version: str,
  last_run_id: str,
  /** Send as `If-Match: "<row_version>"` with `updateExperiment`. */
  row_version: z.number().int(),
  created_by: str,
  created_at: z.string(),
  updated_at: z.string(),
});
export type Experiment = z.infer<typeof experimentSchema>;

export const experimentPageSchema = z.object({ items: z.array(experimentSchema), next_cursor: z.string().nullable().optional() });
export type ExperimentPage = z.infer<typeof experimentPageSchema>;

export const createExperimentRequestSchema = z.object({
  name: z.string(),
  description: str,
  from_experiment_id: str,
  from_version: str,
});
export type CreateExperimentRequest = z.infer<typeof createExperimentRequestSchema>;

export const updateExperimentRequestSchema = z.object({
  name: str,
  description: str,
  profile: profileSchema.nullable().optional(),
  baselines_version: str,
});
export type UpdateExperimentRequest = z.infer<typeof updateExperimentRequestSchema>;

export const promoteExperimentRequestSchema = z.object({
  kind: z.enum(["stimulus_catalog", "normative_baselines", "scoring_profile"]),
  version: z.string(),
  notes: str,
});
export type PromoteExperimentRequest = z.infer<typeof promoteExperimentRequestSchema>;

// ---- cohorts (COH-1..3) ---------------------------------------------------------------------------------------

export const SEXES = ["female", "male", "other", "unknown"] as const;

export const cohortDefinitionSchema = z.object({
  age_bands: z.array(z.string()).optional(),
  /** Applied only once sessions carry sex (ADR-0012 Q8); never fabricated. */
  sex: z.array(z.enum(SEXES)).optional(),
  device_ids: z.array(z.string()).optional(),
  from: str,
  to: str,
  quality_verdicts: z.array(z.string()).optional(),
  tracking_verdicts: z.array(z.string()).optional(),
  min_valid_ratio: optNum,
  scoring_status: z.array(z.string()).optional(),
  require_frames: z.boolean().optional(),
  include_synthetic: z.boolean().optional(),
});
export type CohortDefinition = z.infer<typeof cohortDefinitionSchema>;

export const cohortSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  name: z.string(),
  definition: cohortDefinitionSchema,
  definition_hash: z.string(),
  created_by: str,
  created_at: z.string(),
  updated_at: z.string(),
  archived_at: str,
});
export type Cohort = z.infer<typeof cohortSchema>;

export const cohortPageSchema = z.object({ items: z.array(cohortSchema), next_cursor: z.string().nullable().optional() });
export type CohortPage = z.infer<typeof cohortPageSchema>;

export const createCohortRequestSchema = z.object({ name: z.string(), definition: cohortDefinitionSchema });
export type CreateCohortRequest = z.infer<typeof createCohortRequestSchema>;

export const updateCohortRequestSchema = z.object({
  name: str,
  definition: cohortDefinitionSchema.nullable().optional(),
  archived: z.boolean().nullable().optional(),
});
export type UpdateCohortRequest = z.infer<typeof updateCohortRequestSchema>;

const levelCountsSchema = z.object({ frames: count, score: count });

/** COH-2 preview. Suppressed counts (PRV-1, below `small_cell_k`) are `null`, never zero. */
export const cohortPreviewSchema = z.object({
  n: optCount,
  by_age_band: z.array(
    z.object({ band: z.string(), n: optCount, frames_n: optCount, suppressed: z.boolean() }),
  ),
  by_level: levelCountsSchema,
  excluded: z.object({ no_consent: count, withdrawn: count, unscorable: count, no_age: count, quality: count }),
  small_cell_k: count,
});
export type CohortPreview = z.infer<typeof cohortPreviewSchema>;

// ---- analysis runs (RUN-1..9, SWP-1..3) -----------------------------------------------------------------------

export const RUN_KINDS = ["cohort_rescore", "param_sweep", "age_profile"] as const;
export const RUN_STATUSES = ["queued", "running", "succeeded", "failed", "cancelled"] as const;
export const LEVEL_REQUESTS = ["auto", "score", "frames"] as const;
export const LEVELS = ["score", "frames"] as const;
export const runStatusSchema = z.enum(RUN_STATUSES);
export type RunStatus = z.infer<typeof runStatusSchema>;

export function isRunFinished(status: RunStatus): boolean {
  return status === "succeeded" || status === "failed" || status === "cancelled";
}

export const sweepAxisSchema = z.object({ param_id: z.string(), from: z.number(), to: z.number(), step: z.number() });
export const sweepSpecSchema = z.object({ axes: z.array(sweepAxisSchema) });
export type SweepSpec = z.infer<typeof sweepSpecSchema>;

/**
 * A run's bootstrap seed as stored: a 64-bit integer, so above 2^53 it arrives as its exact decimal text (`parseApiJson`
 * in `live.ts`) and is shown as such; a seed the console sends is always a safe integer.
 */
export const storedSeedSchema = z.union([z.number().int(), z.string().regex(/^-?\d+$/)]);

export const runOptionsSchema = z.object({ metrics: z.array(z.string()).optional(), bootstrap: z.boolean().optional() });

export const analysisRunRequestSchema = z.object({
  kind: z.enum(RUN_KINDS),
  experiment_id: z.string(),
  cohort_id: str,
  cohort: cohortDefinitionSchema.nullable().optional(),
  /** WP-X.11: run on a frozen dataset's members instead of a cohort (exactly one of the three). */
  dataset_id: str,
  sweep: sweepSpecSchema.nullable().optional(),
  level: z.enum(LEVEL_REQUESTS).optional(),
  seed: z.number().int().nullable().optional(),
  options: runOptionsSchema.optional(),
});
export type AnalysisRunRequest = z.infer<typeof analysisRunRequestSchema>;

export const analysisDefinitionSchema = z.object({
  kind: z.enum(RUN_KINDS),
  cohort: cohortDefinitionSchema,
  profile: profileSchema,
  baselines_version: str,
  sweep: sweepSpecSchema.nullable().optional(),
  level: z.enum(LEVEL_REQUESTS),
  options: runOptionsSchema,
  seed: storedSeedSchema,
  /** WP-X.11: the dataset whose frozen members the run read. */
  dataset: z.object({ id: z.string(), content_hash: z.string() }).nullable().optional(),
});

/** RUN-4 distribution. `suppressed: true` means every number is `null` (show "n < k", never 0). */
export const distSchema = z.object({
  n: optCount,
  suppressed: z.boolean().optional(),
  mean: optNum,
  sd: optNum,
  median: optNum,
  iqr: pair.nullable().optional(),
  p10: optNum,
  p90: optNum,
  ci95_mean: pair.nullable().optional(),
  hist: z.object({ edges: z.array(z.number()), counts: z.array(count) }).nullable().optional(),
});
export type Dist = z.infer<typeof distSchema>;

const distComparisonSchema = z.object({ published: distSchema, draft: distSchema, delta: distSchema });
const flagCountsSchema = z.object({ green: optCount, yellow: optCount, red: optCount, unreliable: optCount });
const flagRatesSchema = z.object({ green: z.number(), yellow: z.number(), red: z.number(), unreliable: z.number() });
/** `[from, to, n]`; `n` is null when PRV-1 suppressed the cell. */
const transitionSchema = z.tuple([z.string(), z.string(), count.nullable()]);
const meanMedianSchema = z.object({ mean: optNum, median: optNum });

export const bandSummarySchema = z.object({
  band: z.string(),
  n: optCount,
  suppressed: z.boolean().optional(),
  metrics: z.record(z.string(), distComparisonSchema),
  composite: distComparisonSchema,
  flag_rate: z.object({ published: flagRatesSchema.nullable().optional(), draft: flagRatesSchema.nullable().optional() }),
});
export type BandSummary = z.infer<typeof bandSummarySchema>;

export const runSummarySchema = z.object({
  counts: z.object({
    sessions: count,
    by_level: levelCountsSchema,
    excluded: z.record(z.string(), count),
    params_not_applied: z.record(z.string(), count),
    suppressed_cells: count.optional(),
  }),
  flags: z.object({ published: flagCountsSchema, draft: flagCountsSchema, transitions: z.array(transitionSchema) }),
  quality: z.object({ transitions: z.array(transitionSchema) }),
  by_age_band: z.array(bandSummarySchema),
  overall: bandSummarySchema,
  sweep: z
    .object({
      axes: z.array(sweepAxisSchema),
      /** The published profile's value per axis: always a cell (the last one when off the grid, SWP-2). */
      published_values: z.array(z.number()).optional(),
      cells: z.array(
        z.object({
          values: z.array(z.number()),
          n: optCount,
          flag_rate: flagRatesSchema.nullable().optional(),
          quality_rate: z.record(z.string(), z.number().nullable()),
          metrics: z.record(z.string(), meanMedianSchema),
          composite: meanMedianSchema,
          changed_vs_published: optCount,
        }),
      ),
    })
    .nullable()
    .optional(),
  /** AGE-1 block (`Value` in the contract); read with `parseAgeProfile` from `lib/research/age-profile`. */
  age_profile: z.unknown().optional(),
  small_cell_k: count,
  includes_synthetic: z.boolean().optional(),
});
export type RunSummary = z.infer<typeof runSummarySchema>;

export const jobProgressSchema = z.object({ done: count, total: count, phase: z.string() });
export type JobProgress = z.infer<typeof jobProgressSchema>;

export const analysisRunSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  kind: z.enum(RUN_KINDS),
  status: runStatusSchema,
  experiment_id: str,
  cohort_id: str,
  /** WP-X.11 provenance: the dataset and its content hash at enqueue time. */
  dataset_id: str,
  dataset_hash: str,
  definition: analysisDefinitionSchema,
  // RUN-5 provenance: every field is shown, copyable, on the run page.
  definition_hash: z.string(),
  cohort_hash: z.string(),
  membership_hash: str,
  profile_hash: z.string(),
  published_profile_version: z.string(),
  baselines_version: z.string(),
  catalog_version: z.string(),
  bundle_version: str,
  core_version: z.string(),
  contract_version: z.string(),
  pipeline_version: z.string(),
  registry_version: z.string(),
  seed: storedSeedSchema,
  level_requested: z.enum(LEVEL_REQUESTS),
  sessions_n: optCount,
  sessions_frames_n: optCount,
  sessions_score_n: optCount,
  excluded: z.record(z.string(), count).nullable().optional(),
  summary: runSummarySchema.nullable().optional(),
  result_bytes: optCount,
  progress: jobProgressSchema.nullable().optional(),
  job_id: str,
  /** Failure reason, e.g. `budget_exceeded` or `result-too-large`. */
  error: str,
  membership_drift: z.object({ added: count, removed: count }).nullable().optional(),
  created_by: str,
  created_at: z.string(),
  started_at: str,
  finished_at: str,
});
export type AnalysisRun = z.infer<typeof analysisRunSchema>;

export const analysisRunPageSchema = z.object({ items: z.array(analysisRunSchema), next_cursor: z.string().nullable().optional() });
export type AnalysisRunPage = z.infer<typeof analysisRunPageSchema>;

const sessionSideSchema = z.object({
  composite: optNum,
  flag: z.enum(["green", "yellow", "red", "unreliable"]).nullable().optional(),
  quality_status: str,
  metrics: z.record(z.string(), z.number().nullable()),
});

/** RUN-3 per-session row (PRV-2: no token, MRN, name or device model). */
export const analysisSessionRowSchema = z.object({
  session_id: z.string(),
  level: z.enum(LEVELS),
  age_months: z.number().int().nullable(),
  age_band: z.string().nullable(),
  device_id: z.string().nullable(),
  published: sessionSideSchema,
  draft: sessionSideSchema,
  delta: z.object({ composite: optNum, metrics: z.record(z.string(), z.number().nullable()) }),
  warnings: z.array(z.string()).optional(),
});
export type AnalysisSessionRow = z.infer<typeof analysisSessionRowSchema>;

export const analysisSessionPageSchema = z.object({
  items: z.array(analysisSessionRowSchema),
  next_cursor: z.string().nullable().optional(),
});
export type AnalysisSessionPage = z.infer<typeof analysisSessionPageSchema>;

// ---- datasets (WP-X.11, ADR-0015 R-7..R-9) ----------------------------------------------------------------------

export const DATASET_STATUSES = ["frozen", "archived"] as const;
export const SPLIT_NAMES = ["train", "validation", "holdout"] as const;
export const LABEL_SOURCE_KINDS = ["review_outcomes", "import"] as const;
export const EXCLUSION_RULE_KINDS = ["quality_gates", "tukey", "mad_z", "explicit_list"] as const;
export const COLUMN_TYPES = ["string", "integer", "number", "datetime"] as const;
export type SplitName = (typeof SPLIT_NAMES)[number];

const explicitExclusionSchema = z.object({ session_id: z.string(), reason: z.string() });

/** An exclusion rule (ADR-0015 V-5 kinds). The backend applies `quality_gates` and `explicit_list` today. */
export const exclusionRuleSchema = z.discriminatedUnion("kind", [
  z.object({
    kind: z.literal("quality_gates"),
    id: z.string(),
    min_valid_ratio: optNum,
    exclude_quality_verdicts: z.array(z.string()).optional(),
    exclude_tracking_verdicts: z.array(z.string()).optional(),
    min_clips_completed: optCount,
  }),
  z.object({ kind: z.literal("tukey"), id: z.string(), metric: z.string(), k: z.number().optional() }),
  z.object({ kind: z.literal("mad_z"), id: z.string(), metric: z.string(), z: z.number().optional() }),
  z.object({ kind: z.literal("explicit_list"), id: z.string(), sessions: z.array(explicitExclusionSchema) }),
]);
export type ExclusionRule = z.infer<typeof exclusionRuleSchema>;

export const exclusionSchema = z.object({ session_id: z.string(), rule_id: str, reason: z.string() });
export type Exclusion = z.infer<typeof exclusionSchema>;

export const labelSourceSchema = z.object({ kind: z.enum(LABEL_SOURCE_KINDS), taken_at: z.string(), count });
export const labelSchema = z.object({
  value: z.string(),
  source: z.enum(LABEL_SOURCE_KINDS),
  taken_at: z.string(),
  reviewer_pseudonym: str,
});
export type Label = z.infer<typeof labelSchema>;

export const splitFractionsSchema = z.object({ train: z.number(), validation: z.number(), holdout: z.number() });
export type SplitFractions = z.infer<typeof splitFractionsSchema>;
export const splitSchema = z.object({ seed: storedSeedSchema, fractions: splitFractionsSchema, strata: z.literal("age_band").optional() });
export type Split = z.infer<typeof splitSchema>;
export const splitRequestSchema = z.object({
  seed: z.number().int().nullable().optional(),
  fractions: splitFractionsSchema.optional(),
  strata: z.literal("age_band").optional(),
});
export const splitStratumCountSchema = z.object({ stratum: z.string(), train: count, validation: count, holdout: count });
export type SplitStratumCount = z.infer<typeof splitStratumCountSchema>;

export const lineageSchema = z.object({
  cohort_id: str,
  cohort_hash: z.string(),
  parent_dataset_id: str,
  run_id: str,
  profile_hash: z.string(),
  profile_version: z.string(),
  catalog_version: z.string(),
  baselines_version: z.string(),
  code_version: z.string(),
});
export type Lineage = z.infer<typeof lineageSchema>;

/** A data dictionary column; `description_key` is a domain catalogue key (`dataset_column.<name>`). */
export const columnSchema = z.object({
  name: z.string(),
  type: z.enum(COLUMN_TYPES),
  unit: unitSchema.nullable().optional(),
  description_key: z.string(),
});
export type Column = z.infer<typeof columnSchema>;

/** A frozen dataset (R-7): immutable; `content_hash` is its identity; erasure only raises `erased_count` (R-9). */
export const datasetSchema = z.object({
  id: z.string(),
  org_id: z.string(),
  name: z.string(),
  description: str,
  status: z.enum(DATASET_STATUSES),
  cohort_id: str,
  frozen_at: z.string(),
  content_hash: z.string(),
  member_count: count,
  erased_count: count.optional(),
  exclusion_rules: z.array(exclusionRuleSchema),
  exclusions: z.array(exclusionSchema),
  label_source: labelSourceSchema.nullable().optional(),
  split: splitSchema.nullable().optional(),
  split_counts: z.array(splitStratumCountSchema).optional(),
  lineage: lineageSchema,
  data_dictionary: z.array(columnSchema),
  created_by: str,
  created_at: z.string(),
  archived_at: str,
});
export type Dataset = z.infer<typeof datasetSchema>;

export const datasetPageSchema = z.object({ items: z.array(datasetSchema), next_cursor: z.string().nullable().optional() });
export type DatasetPage = z.infer<typeof datasetPageSchema>;

/** One member; an erased one keeps only `session_id` and `excluded: "erased"`. */
export const datasetMemberSchema = z.object({
  session_id: z.string(),
  participant_pseudonym: str,
  age_months: z.number().int().nullable().optional(),
  age_band: str,
  split: z.enum(SPLIT_NAMES).nullable().optional(),
  label: labelSchema.nullable().optional(),
  excluded: str,
});
export type DatasetMember = z.infer<typeof datasetMemberSchema>;

export const datasetMemberPageSchema = z.object({ items: z.array(datasetMemberSchema), next_cursor: z.string().nullable().optional() });
export type DatasetMemberPage = z.infer<typeof datasetMemberPageSchema>;

export const createDatasetRequestSchema = z.object({
  name: z.string(),
  description: str,
  cohort_id: z.string(),
  parent_dataset_id: str,
  run_id: str,
  exclusion_rules: z.array(exclusionRuleSchema).optional(),
  labels: z.enum(LABEL_SOURCE_KINDS).nullable().optional(),
  split: splitRequestSchema.nullable().optional(),
});
export type CreateDatasetRequest = z.infer<typeof createDatasetRequestSchema>;

export interface DatasetMemberQuery extends ResearchListQuery {
  split?: SplitName;
}

// ---- effective configuration (GOV-6) ------------------------------------------------------------------------------

const effectiveDocumentSchema = z.object({ version: z.string(), body: z.unknown() });
export const effectiveConfigSchema = z.object({
  source: z.enum(["org_bundle", "global"]),
  bundle_version: str,
  scoring_profile: effectiveDocumentSchema,
  normative_baselines: effectiveDocumentSchema,
  stimulus_catalog: effectiveDocumentSchema,
});
export type EffectiveConfig = z.infer<typeof effectiveConfigSchema>;

// ---- client -------------------------------------------------------------------------------------------------------

export interface ResearchListQuery {
  cursor?: string;
  limit?: number;
  status?: string;
}

export interface AnalysisRunListQuery extends ResearchListQuery {
  kind?: string;
  experiment_id?: string;
}

export interface ResearchApi {
  /** `GET /v1/orgs/{id}/experiments` (`AnalysisRead`). */
  listExperiments(orgId: string, query?: ResearchListQuery): Promise<ExperimentPage>;
  /** `POST /v1/orgs/{id}/experiments` (`ExperimentWrite`); `409 experiment-name-exists`. */
  createExperiment(orgId: string, request: CreateExperimentRequest): Promise<Experiment>;
  getExperiment(experimentId: string): Promise<Experiment>;
  /** `PATCH /v1/experiments/{id}` with `If-Match`; `412 stale-version`, `400 config-invalid`. */
  updateExperiment(experimentId: string, rowVersion: number, request: UpdateExperimentRequest): Promise<Experiment>;
  archiveExperiment(experimentId: string): Promise<Experiment>;
  /**
   * `POST /v1/experiments/{id}/promote` (`ExperimentPromote`) → `201` with the draft `ConfigDocument` it filed (the
   * experiment becomes `promoted`); `409 experiment-no-run | experiment-promoted`.
   */
  promoteExperiment(experimentId: string, request: PromoteExperimentRequest): Promise<ConfigDocument>;
  listCohorts(orgId: string, query?: ResearchListQuery): Promise<CohortPage>;
  createCohort(orgId: string, request: CreateCohortRequest): Promise<Cohort>;
  getCohort(cohortId: string): Promise<Cohort>;
  updateCohort(cohortId: string, request: UpdateCohortRequest): Promise<Cohort>;
  /** `POST /v1/orgs/{id}/cohorts/preview` (`AnalysisRead`): counts only, suppressed below `small_cell_k`. */
  previewCohort(orgId: string, definition: CohortDefinition): Promise<CohortPreview>;
  listAnalysisRuns(orgId: string, query?: AnalysisRunListQuery): Promise<AnalysisRunPage>;
  /** `POST /v1/orgs/{id}/analysis-runs` (`AnalysisRun`) → `202`; `429 analysis-quota-exceeded`, `400 *-too-large`. */
  createAnalysisRun(orgId: string, request: AnalysisRunRequest): Promise<AnalysisRun>;
  getAnalysisRun(runId: string): Promise<AnalysisRun>;
  listAnalysisRunSessions(runId: string, query?: ResearchListQuery): Promise<AnalysisSessionPage>;
  cancelAnalysisRun(runId: string): Promise<AnalysisRun>;
  rerunAnalysisRun(runId: string): Promise<AnalysisRun>;
  /** `GET /v1/orgs/{id}/config/effective` (`ConfigRead`, GOV-6). */
  getEffectiveConfig(orgId: string): Promise<EffectiveConfig>;
  /** WP-X.11 `GET /v1/research/datasets` (`AnalysisRead`): the caller's org, newest first. */
  listDatasets(orgId: string, query?: ResearchListQuery): Promise<DatasetPage>;
  /** `POST /v1/research/datasets` (`ExperimentWrite`) → `201`; `400 cohort-empty | cohort-too-large`, `409 conflict`. */
  createDataset(orgId: string, request: CreateDatasetRequest): Promise<Dataset>;
  getDataset(datasetId: string): Promise<Dataset>;
  /** `GET /v1/research/datasets/{id}/members` (`AnalysisRead` + `SessionRead`); audited once per page. */
  listDatasetMembers(datasetId: string, query?: DatasetMemberQuery): Promise<DatasetMemberPage>;
  /** `POST /v1/research/datasets/{id}/archive`; `409 invalid-transition` when already archived. */
  archiveDataset(datasetId: string): Promise<Dataset>;
  /**
   * RFL-4 `GET /v1/sessions/{id}/research-frames` (`FramesRead`): the research frame log as JSON Lines text, never
   * parsed here (the Rust core reprocesses it). Audited `session.frames_read` per call; `404 frames-unavailable`,
   * `410 frames-purged`.
   */
  getResearchFrames(sessionId: string): Promise<string>;
}

export function createLiveResearch(
  request: LiveRequest,
  buildQuery: (q: object) => string,
  /** GET returning the body as text (`live.ts`), for the one route that is not JSON. */
  requestText: (path: string) => Promise<string>,
): ResearchApi {
  const enc = encodeURIComponent;
  return {
    getResearchFrames: (id) => requestText(`/sessions/${enc(id)}/research-frames`),
    listExperiments: (orgId, q) =>
      request(`/orgs/${enc(orgId)}/experiments${buildQuery(q ?? {})}`, experimentPageSchema),
    createExperiment: (orgId, body) =>
      request(`/orgs/${enc(orgId)}/experiments`, experimentSchema, { method: "POST", body }),
    getExperiment: (id) => request(`/experiments/${enc(id)}`, experimentSchema),
    updateExperiment: (id, rowVersion, body) =>
      request(`/experiments/${enc(id)}`, experimentSchema, {
        method: "PATCH",
        body,
        headers: { "If-Match": `"${rowVersion}"` },
      }),
    archiveExperiment: (id) => request(`/experiments/${enc(id)}/archive`, experimentSchema, { method: "POST", body: {} }),
    promoteExperiment: (id, body) => request(`/experiments/${enc(id)}/promote`, configDocumentSchema, { method: "POST", body }),
    listCohorts: (orgId, q) => request(`/orgs/${enc(orgId)}/cohorts${buildQuery(q ?? {})}`, cohortPageSchema),
    createCohort: (orgId, body) => request(`/orgs/${enc(orgId)}/cohorts`, cohortSchema, { method: "POST", body }),
    getCohort: (id) => request(`/cohorts/${enc(id)}`, cohortSchema),
    updateCohort: (id, body) => request(`/cohorts/${enc(id)}`, cohortSchema, { method: "PATCH", body }),
    previewCohort: (orgId, body) =>
      request(`/orgs/${enc(orgId)}/cohorts/preview`, cohortPreviewSchema, { method: "POST", body }),
    listAnalysisRuns: (orgId, q) =>
      request(`/orgs/${enc(orgId)}/analysis-runs${buildQuery(q ?? {})}`, analysisRunPageSchema),
    createAnalysisRun: (orgId, body) =>
      request(`/orgs/${enc(orgId)}/analysis-runs`, analysisRunSchema, { method: "POST", body }),
    getAnalysisRun: (id) => request(`/analysis-runs/${enc(id)}`, analysisRunSchema),
    listAnalysisRunSessions: (id, q) =>
      request(`/analysis-runs/${enc(id)}/sessions${buildQuery(q ?? {})}`, analysisSessionPageSchema),
    cancelAnalysisRun: (id) => request(`/analysis-runs/${enc(id)}/cancel`, analysisRunSchema, { method: "POST", body: {} }),
    rerunAnalysisRun: (id) => request(`/analysis-runs/${enc(id)}/rerun`, analysisRunSchema, { method: "POST", body: {} }),
    getEffectiveConfig: (orgId) => request(`/orgs/${enc(orgId)}/config/effective`, effectiveConfigSchema),
    // The org is the caller's (the token's); `orgId` keeps the interface uniform and is not sent.
    listDatasets: (_orgId, q) => request(`/research/datasets${buildQuery(q ?? {})}`, datasetPageSchema),
    createDataset: (_orgId, body) => request(`/research/datasets`, datasetSchema, { method: "POST", body }),
    getDataset: (id) => request(`/research/datasets/${enc(id)}`, datasetSchema),
    listDatasetMembers: (id, q) =>
      request(`/research/datasets/${enc(id)}/members${buildQuery(q ?? {})}`, datasetMemberPageSchema),
    archiveDataset: (id) => request(`/research/datasets/${enc(id)}/archive`, datasetSchema, { method: "POST", body: {} }),
  };
}
