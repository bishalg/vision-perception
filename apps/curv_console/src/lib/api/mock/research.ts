/**
 * Mock researcher toolkit (RUI-7, WP-X.0 step 0). Synthetic data only. Seeds two experiments, two cohorts and five
 * analysis runs (one running with progress, one succeeded with a suppressed band, one failed `budget_exceeded`, and
 * from WP-X.9 a succeeded two-axis `param_sweep` of 625 grid cells plus the off-grid published cell, and a succeeded
 * `age_profile` with a suppressed band) so the research pages and Playwright cover every state without a backend. It mimics the answers the UI surfaces
 * (`409 experiment-name-exists | experiment-no-run | experiment-promoted`, `412 stale-version`, `404`); the real rules,
 * evaluation, suppression and audit live in the Rust backend and core.
 */
import { ApiError, PROBLEM_BASE } from "../types";
import type {
  AnalysisRun,
  AnalysisSessionRow,
  BandSummary,
  Cohort,
  CohortDefinition,
  Dist,
  Experiment,
  ResearchApi,
  RunSummary,
} from "../research";

export interface MockResearchContext {
  orgId: string;
  userId: string;
  /** Epoch milliseconds (`MOCK_FIXED_NOW`). */
  now: number;
}

/** Seeded ids, stable for tests and e2e. */
export const MOCK_RESEARCH_IDS = {
  experimentDraft: "e0000000-0000-4000-8000-000000000001",
  experimentPromoted: "e0000000-0000-4000-8000-000000000002",
  cohortToddlers: "c0000000-0000-4000-8000-000000000001",
  cohortAll: "c0000000-0000-4000-8000-000000000002",
  runRunning: "a0000000-0000-4000-8000-000000000001",
  runSucceeded: "a0000000-0000-4000-8000-000000000002",
  runFailed: "a0000000-0000-4000-8000-000000000003",
  runSweep: "a0000000-0000-4000-8000-000000000004",
  runAgeProfile: "a0000000-0000-4000-8000-000000000005",
} as const;

const SMALL_CELL_K = 12;
const HASH = (c: string) => c.repeat(64);
/** Mock run progression (see `advance`): polls until `running` and until `succeeded`. */
const PROGRESS_POLLS_TO_RUN = 2;
const PROGRESS_POLLS_TO_SUCCEED = 5;

/**
 * A `NormativeBaselineSet`-shaped body (config-bundle-v1.json) with one range, so the run histograms shade it for
 * the 24-35 band. Synthetic: not a norm.
 */
function baselineBody(): Record<string, unknown> {
  return {
    schemaVersion: "1.0.0", status: "provisional-literature-seeded", provenance: "Mock",
    ageBrackets: [{ id: "24-35", minMonths: 24, maxMonths: 35 }, { id: "36-47", minMonths: 36, maxMonths: 47 }],
    videos: {
      mock_clip: {
        title: "Mock clip", category: "Social", weight: 1,
        segments: [{
          id: "full", description: "Whole clip", startSec: 0, endSec: 8,
          expectations: [{
            biomarker: "eye_dwell_ratio", direction: "higher", citation: "Synthetic", source: "literature",
            byAge: { "24-35": { acceptableLow: 0.25, acceptableHigh: 0.5, expected: 0.38 } },
          }],
        }],
      },
    },
  };
}

function problem(status: number, slug: string, title: string, detail?: string) {
  return new ApiError(status, { type: `${PROBLEM_BASE}${slug}`, title, status, detail });
}
const notFound = () => problem(404, "not-found", "Not found");

/** A 1.2.0 profile with the registry defaults that differ from zero (opaque here: the Rust core decodes the rest). */
function profile(ivt: number): Record<string, unknown> {
  return {
    schemaVersion: "1.2.0",
    flag: { greenFloor: 0.65, yellowFloor: 0.35, lowConfidenceShare: 0.5, atypicalShareForRed: 0.4 },
    lowConfidenceValidRatio: 0.6,
    qualityGate: { minDistanceCm: 35, maxDistanceCm: 65, maxYawRad: 0.61, maxPitchRad: 0.52, minConfidence: 0.5, blinkThreshold: 0.45 },
    metrics: { ivtVelocityUnitsPerSec: ivt, minFixationDurationSec: 0.08 },
  };
}

function dist(n: number, mean: number): Dist {
  return {
    n, suppressed: false, mean, sd: 0.08, median: mean, iqr: [mean - 0.05, mean + 0.05],
    p10: n >= 20 ? mean - 0.1 : null, p90: n >= 20 ? mean + 0.1 : null, ci95_mean: [mean - 0.03, mean + 0.03],
    hist: { edges: [0, 0.25, 0.5, 0.75, 1], counts: [Math.floor(n / 4), Math.floor(n / 4), Math.floor(n / 4), n - 3 * Math.floor(n / 4)] },
  };
}
const suppressed = (): Dist => ({ n: null, suppressed: true, mean: null, sd: null, median: null, iqr: null, p10: null, p90: null, ci95_mean: null, hist: null });

function band(id: string, n: number): BandSummary {
  if (n < SMALL_CELL_K) {
    return {
      band: id, n: null, suppressed: true, metrics: {},
      composite: { published: suppressed(), draft: suppressed(), delta: suppressed() },
      flag_rate: { published: null, draft: null },
    };
  }
  return {
    band: id, n, suppressed: false,
    metrics: { eye_dwell_ratio: { published: dist(n, 0.36), draft: dist(n, 0.39), delta: dist(n, 0.03) } },
    composite: { published: dist(n, 0.61), draft: dist(n, 0.64), delta: dist(n, 0.03) },
    flag_rate: {
      published: { green: 0.5, yellow: 0.3, red: 0.15, unreliable: 0.05 },
      draft: { green: 0.55, yellow: 0.28, red: 0.12, unreliable: 0.05 },
    },
  };
}

function succeededSummary(): RunSummary {
  return {
    counts: {
      sessions: 37, by_level: { frames: 10, score: 27 }, excluded: { no_age: 2, quality: 1 },
      params_not_applied: { "metrics.ivtVelocityUnitsPerSec": 27 }, suppressed_cells: 1,
    },
    flags: {
      published: { green: 18, yellow: 12, red: null, unreliable: null },
      draft: { green: 20, yellow: 13, red: null, unreliable: null },
      transitions: [["green", "green", 17], ["yellow", "green", null], ["yellow", "yellow", 12]],
    },
    quality: { transitions: [["ok", "ok", 37]] },
    by_age_band: [band("24-35", 30), band("36-47", 7)],
    overall: band("overall", 37),
    sweep: null,
    small_cell_k: SMALL_CELL_K,
    includes_synthetic: false,
  };
}

/** Synthetic sweep axes of the seeded `param_sweep` run (25 x 25 cells). */
const SWEEP_AXES = [
  { param_id: "flag.greenFloor", from: 0.4, to: 0.88, step: 0.02 },
  { param_id: "flag.yellowFloor", from: 0.1, to: 0.58, step: 0.02 },
];
const SWEEP_STEPS = 25;
const SWEEP_PUBLISHED = [0.65, 0.35];

/** A synthetic sweep cell; the first two values of each axis together are hidden (n < k) to show suppression. */
function sweepCell(values: number[], i: number, j: number): NonNullable<RunSummary["sweep"]>["cells"][number] {
  if (i < 2 && j < 2) {
    return { values, n: null, flag_rate: null, quality_rate: { good: null, low_quality: null, low_tracking_quality: null, unreliable: null }, metrics: {}, composite: { mean: null, median: null }, changed_vs_published: null };
  }
  const [g, y] = values as [number, number];
  const green = Math.max(0, Math.min(1, 1.05 - g));
  const red = Math.max(0, Math.min(1 - green, y * 0.4));
  return {
    values, n: 37,
    flag_rate: { green, yellow: Math.max(0, 1 - green - red - 0.05), red, unreliable: 0.05 },
    quality_rate: { good: 0.8, low_quality: 0.1, low_tracking_quality: 0.05, unreliable: 0.05 },
    metrics: {}, composite: { mean: 0.6 + (i - j) * 0.002, median: 0.61 + (i - j) * 0.002 },
    changed_vs_published: i === 12 && j === 12 ? 0 : 12 + ((i + j) % 9),
  };
}

function sweepSummary(): RunSummary {
  const axisValues = (a: (typeof SWEEP_AXES)[number]) =>
    Array.from({ length: SWEEP_STEPS }, (_, i) => Number((a.from + i * a.step).toFixed(2)));
  const [xs, ys] = [axisValues(SWEEP_AXES[0]!), axisValues(SWEEP_AXES[1]!)];
  const cells = xs.flatMap((x, i) => ys.map((y, j) => sweepCell([x, y], i, j)));
  cells.push(sweepCell([...SWEEP_PUBLISHED], 12, 12));
  return { ...succeededSummary(), sweep: { axes: SWEEP_AXES, published_values: SWEEP_PUBLISHED, cells } };
}

function ageSide(n: number | null, onTarget: number, median: number): Record<string, unknown> {
  if (n === null) {
    return { n: null, suppressed: true, dist: suppressed(), range: { expected: 0.38, acceptable_low: 0.25, acceptable_high: 0.5 }, shares: null, expected_percentile: null };
  }
  return {
    n, suppressed: false, dist: dist(n, median), range: { expected: 0.38, acceptable_low: 0.25, acceptable_high: 0.5 },
    shares: { on_target: onTarget, monitor: Number((0.8 - onTarget).toFixed(2)), atypical: 0.2 }, expected_percentile: 55,
  };
}

function ageProfileSummary(): RunSummary {
  const expectation = (n: number | null, draftOnTarget: number) => ({
    stimulus_id: "mock_clip", segment_id: "full", biomarker: "eye_dwell_ratio", target_aoi: null,
    metric_key: "eye_dwell_ratio", direction: "higherBetter",
    published: ageSide(n, 0.5, 0.36), draft: ageSide(n, draftOnTarget, 0.36),
  });
  return {
    ...succeededSummary(),
    age_profile: {
      baselines: { published: "2026.10.0", draft: "2026.11.0-draft" }, min_n: SMALL_CELL_K, suppression_threshold: SMALL_CELL_K,
      bands: [
        { band: "24-35", min_months: 24, max_months: 35, n: 30, suppressed: false, expectations: [expectation(30, 0.3)] },
        { band: "36-47", min_months: 36, max_months: 47, n: null, suppressed: true, expectations: [expectation(null, 0)] },
      ],
    },
  };
}

const DEFINITION: CohortDefinition = { age_bands: ["24-35", "36-47"], tracking_verdicts: ["good", "fair"], from: "2025-11-01" };

export function createMockResearch(ctx: MockResearchContext): ResearchApi {
  let seq = 100;
  const iso = (offsetMin: number) => new Date(ctx.now + offsetMin * 60_000).toISOString();
  const stamp = () => iso(++seq);
  const snap = <T,>(v: T): T => structuredClone(v);
  const ensureOrg = (orgId: string) => {
    if (orgId !== ctx.orgId) throw notFound();
  };

  const experiments = new Map<string, Experiment>();
  const cohorts = new Map<string, Cohort>();
  const runs = new Map<string, AnalysisRun>();
  const rows = new Map<string, AnalysisSessionRow[]>();

  const experiment = (id: string, over: Partial<Experiment>): Experiment => ({
    id, org_id: ctx.orgId, name: "", description: null, status: "draft", base_profile_version: "1.1.0",
    profile: profile(5), profile_hash: HASH("a"), baselines_version: null, promoted_config_version: null,
    last_run_id: null, row_version: 1, created_by: ctx.userId, created_at: iso(-600), updated_at: iso(-600), ...over,
  });
  experiments.set(MOCK_RESEARCH_IDS.experimentDraft, experiment(MOCK_RESEARCH_IDS.experimentDraft, {
    name: "Lower I-VT threshold", description: "Fixation threshold 4.0 instead of 5.0", profile: profile(4),
    profile_hash: HASH("b"), last_run_id: MOCK_RESEARCH_IDS.runSucceeded,
  }));
  experiments.set(MOCK_RESEARCH_IDS.experimentPromoted, experiment(MOCK_RESEARCH_IDS.experimentPromoted, {
    name: "Stricter green floor", status: "promoted", promoted_config_version: "1.2.0-exp2",
    last_run_id: MOCK_RESEARCH_IDS.runFailed, created_at: iso(-1200), updated_at: iso(-900),
  }));

  const cohort = (id: string, name: string, definition: CohortDefinition): Cohort => ({
    id, org_id: ctx.orgId, name, definition, definition_hash: HASH(id.slice(0, 1) === "c" ? "c" : "d"),
    created_by: ctx.userId, created_at: iso(-700), updated_at: iso(-700), archived_at: null,
  });
  cohorts.set(MOCK_RESEARCH_IDS.cohortToddlers, cohort(MOCK_RESEARCH_IDS.cohortToddlers, "24-47 months, good/fair tracking", DEFINITION));
  cohorts.set(MOCK_RESEARCH_IDS.cohortAll, cohort(MOCK_RESEARCH_IDS.cohortAll, "All consented sessions", {}));

  const run = (id: string, over: Partial<AnalysisRun>): AnalysisRun => ({
    id, org_id: ctx.orgId, kind: "cohort_rescore", status: "queued",
    experiment_id: MOCK_RESEARCH_IDS.experimentDraft, cohort_id: MOCK_RESEARCH_IDS.cohortToddlers,
    definition: { kind: "cohort_rescore", cohort: DEFINITION, profile: profile(4), baselines_version: null, sweep: null, level: "auto", options: { metrics: [], bootstrap: true }, seed: 42 },
    definition_hash: HASH("1"), cohort_hash: HASH("c"), membership_hash: null, profile_hash: HASH("b"),
    published_profile_version: "1.1.0", baselines_version: "2026.10.0", catalog_version: "1.4.0", bundle_version: null,
    core_version: "0.1.0", contract_version: "1", pipeline_version: "1", registry_version: "1.0.0", seed: 42,
    level_requested: "auto", sessions_n: null, sessions_frames_n: null, sessions_score_n: null, excluded: null,
    summary: null, result_bytes: null, progress: null, job_id: `${id.slice(0, 8)}-0000-4000-8000-0000000000b1`,
    error: null, membership_drift: null, created_by: ctx.userId, created_at: iso(-60), started_at: null, finished_at: null,
    ...over,
  });
  runs.set(MOCK_RESEARCH_IDS.runRunning, run(MOCK_RESEARCH_IDS.runRunning, {
    status: "running", started_at: iso(-5), created_at: iso(-6), progress: { done: 120, total: 300, phase: "evaluate" },
    membership_hash: HASH("e"), sessions_n: 300,
  }));
  runs.set(MOCK_RESEARCH_IDS.runSucceeded, run(MOCK_RESEARCH_IDS.runSucceeded, {
    status: "succeeded", membership_hash: HASH("f"), sessions_n: 37, sessions_frames_n: 10, sessions_score_n: 27,
    excluded: { no_age: 2, quality: 1 }, summary: succeededSummary(), result_bytes: 18_432,
    progress: { done: 37, total: 37, phase: "done" }, created_at: iso(-120), started_at: iso(-119), finished_at: iso(-110),
  }));
  runs.set(MOCK_RESEARCH_IDS.runFailed, run(MOCK_RESEARCH_IDS.runFailed, {
    status: "failed", experiment_id: MOCK_RESEARCH_IDS.experimentPromoted, cohort_id: MOCK_RESEARCH_IDS.cohortAll,
    level_requested: "frames", error: "budget_exceeded", created_at: iso(-300), started_at: iso(-299), finished_at: iso(-269),
  }));
  runs.set(MOCK_RESEARCH_IDS.runSweep, run(MOCK_RESEARCH_IDS.runSweep, {
    kind: "param_sweep", status: "succeeded", membership_hash: HASH("f"), sessions_n: 37, sessions_frames_n: 0,
    sessions_score_n: 37, level_requested: "score", excluded: { no_age: 2 }, summary: sweepSummary(), result_bytes: 18_432,
    definition: {
      kind: "param_sweep", cohort: DEFINITION, profile: profile(4), baselines_version: null,
      sweep: { axes: SWEEP_AXES }, level: "score", options: { metrics: [], bootstrap: true }, seed: 42,
    },
    progress: { done: 37, total: 37, phase: "done" }, created_at: iso(-90), started_at: iso(-89), finished_at: iso(-80),
  }));
  runs.set(MOCK_RESEARCH_IDS.runAgeProfile, run(MOCK_RESEARCH_IDS.runAgeProfile, {
    kind: "age_profile", status: "succeeded", membership_hash: HASH("f"), sessions_n: 37, sessions_frames_n: 10,
    sessions_score_n: 27, baselines_version: "2026.11.0-draft", excluded: { no_age: 2 }, summary: ageProfileSummary(),
    definition: {
      kind: "age_profile", cohort: DEFINITION, profile: profile(5), baselines_version: "2026.11.0-draft",
      sweep: null, level: "auto", options: { metrics: [], bootstrap: true }, seed: 42,
    },
    result_bytes: 18_432, progress: { done: 37, total: 37, phase: "done" }, created_at: iso(-100), started_at: iso(-99),
    finished_at: iso(-95),
  }));
  const sessionRows = (): AnalysisSessionRow[] => Array.from({ length: 37 }, (_, i): AnalysisSessionRow => {
    const composite = 0.45 + (i % 10) * 0.04;
    return {
      session_id: `SESS-MOCK-${String(i + 1).padStart(4, "0")}`, level: i < 10 ? "frames" : "score",
      age_months: 24 + (i % 24), age_band: i % 24 < 12 ? "24-35" : "36-47", device_id: null,
      published: { composite, flag: composite >= 0.65 ? "green" : "yellow", quality_status: "ok", metrics: { eye_dwell_ratio: 0.36 } },
      draft: { composite: composite + 0.03, flag: composite + 0.03 >= 0.65 ? "green" : "yellow", quality_status: "ok", metrics: { eye_dwell_ratio: 0.39 } },
      delta: { composite: 0.03, metrics: { eye_dwell_ratio: 0.03 } },
      warnings: i < 10 ? [] : ["param_not_applied: metrics.ivtVelocityUnitsPerSec"],
    };
  });
  rows.set(MOCK_RESEARCH_IDS.runSucceeded, sessionRows());

  function page<T extends { id?: string; created_at?: string }>(items: T[], cursor?: string, limit = 20) {
    const start = cursor ? Number.parseInt(cursor, 10) || 0 : 0;
    const slice = items.slice(start, start + limit);
    return { items: snap(slice), next_cursor: start + limit < items.length ? String(start + limit) : null };
  }
  const newest = <T extends { created_at: string; id: string }>(m: Map<string, T>) =>
    [...m.values()].sort((a, b) => b.created_at.localeCompare(a.created_at) || b.id.localeCompare(a.id));

  const getExperiment = (id: string) => {
    const e = experiments.get(id);
    if (!e) throw notFound();
    return e;
  };
  const getRun = (id: string) => {
    const r = runs.get(id);
    if (!r) throw notFound();
    return r;
  };
  /**
   * Deterministic progression for the UI (no timers): every `getAnalysisRun` of a queued or running run counts as one
   * poll. The 2nd poll moves it to `running` with progress, the 5th to `succeeded` with the seeded summary. The seeded
   * running run (`MOCK_RESEARCH_IDS.runRunning`) never advances, so "cancel while running" has a stable subject.
   */
  const polls = new Map<string, number>();
  const advance = (r: AnalysisRun) => {
    if (r.id === MOCK_RESEARCH_IDS.runRunning || (r.status !== "queued" && r.status !== "running")) return;
    const n = (polls.get(r.id) ?? 0) + 1;
    polls.set(r.id, n);
    if (n >= PROGRESS_POLLS_TO_SUCCEED) {
      Object.assign(r, {
        status: "succeeded", membership_hash: HASH("f"), sessions_n: 37, sessions_frames_n: 10, sessions_score_n: 27,
        excluded: { no_age: 2, quality: 1 }, summary: succeededSummary(), result_bytes: 18_432,
        progress: { done: 37, total: 37, phase: "done" }, started_at: r.started_at ?? stamp(), finished_at: stamp(),
      } satisfies Partial<AnalysisRun>);
      rows.set(r.id, sessionRows());
    } else if (n >= PROGRESS_POLLS_TO_RUN) {
      const done = Math.round((37 * (n - PROGRESS_POLLS_TO_RUN + 1)) / (PROGRESS_POLLS_TO_SUCCEED - PROGRESS_POLLS_TO_RUN + 1));
      Object.assign(r, { status: "running", started_at: r.started_at ?? stamp(), progress: { done, total: 37, phase: "evaluate" } });
    }
  };
  const nameTaken =(name: string, except?: string) =>
    [...experiments.values()].some((e) => e.name === name && e.id !== except);
  const newId = (prefix: string) => `${prefix}0000000-0000-4000-8000-${String(++seq).padStart(12, "0")}`;

  return {
    async listExperiments(orgId, q) {
      ensureOrg(orgId);
      return page(newest(experiments).filter((e) => !q?.status || e.status === q.status), q?.cursor, q?.limit);
    },
    async createExperiment(orgId, req) {
      ensureOrg(orgId);
      if (nameTaken(req.name)) throw problem(409, "experiment-name-exists", "Name exists", "an experiment with that name exists");
      const from = req.from_experiment_id ? getExperiment(req.from_experiment_id) : null;
      const e = experiment(newId("e"), {
        name: req.name, description: req.description ?? null, profile: snap(from?.profile ?? profile(5)),
        created_at: stamp(), updated_at: stamp(),
      });
      experiments.set(e.id, e);
      return snap(e);
    },
    async getExperiment(id) {
      return snap(getExperiment(id));
    },
    async updateExperiment(id, rowVersion, req) {
      const e = getExperiment(id);
      if (e.row_version !== rowVersion) throw problem(412, "stale-version", "Stale version", "the experiment changed; reload it");
      if (req.name && nameTaken(req.name, id)) throw problem(409, "experiment-name-exists", "Name exists");
      Object.assign(e, {
        ...(req.name ? { name: req.name } : {}),
        ...(req.description !== undefined ? { description: req.description } : {}),
        ...(req.profile ? { profile: snap(req.profile), profile_hash: HASH(String(++seq % 10)) } : {}),
        ...(req.baselines_version !== undefined ? { baselines_version: req.baselines_version } : {}),
        row_version: e.row_version + 1,
        updated_at: stamp(),
      });
      return snap(e);
    },
    async archiveExperiment(id) {
      const e = getExperiment(id);
      e.status = "archived";
      e.updated_at = stamp();
      return snap(e);
    },
    async promoteExperiment(id, req) {
      const e = getExperiment(id);
      if (e.status === "promoted") throw problem(409, "experiment-promoted", "Already promoted");
      if (!e.last_run_id) throw problem(409, "experiment-no-run", "No run", "run the experiment on a cohort first");
      const at = stamp();
      Object.assign(e, { status: "promoted", promoted_config_version: req.version, updated_at: at });
      return {
        id: newId("d"), kind: req.kind, version: req.version, status: "draft", body: snap(e.profile),
        created_by: e.created_by ?? null, created_at: at, published_at: null,
        notes: `experiment:${e.id} run:${e.last_run_id}${req.notes ? ` ${req.notes}` : ""}`,
      };
    },
    async listCohorts(orgId, q) {
      ensureOrg(orgId);
      return page(newest(cohorts), q?.cursor, q?.limit);
    },
    async createCohort(orgId, req) {
      ensureOrg(orgId);
      if ([...cohorts.values()].some((c) => c.name === req.name)) throw problem(409, "conflict", "Conflict", "a cohort with that name exists");
      const c = cohort(newId("c"), req.name, snap(req.definition));
      c.created_at = c.updated_at = stamp();
      cohorts.set(c.id, c);
      return snap(c);
    },
    async getCohort(id) {
      const c = cohorts.get(id);
      if (!c) throw notFound();
      return snap(c);
    },
    async updateCohort(id, req) {
      const c = cohorts.get(id);
      if (!c) throw notFound();
      if (req.name) c.name = req.name;
      if (req.definition) c.definition = snap(req.definition);
      if (req.archived) c.archived_at = stamp();
      c.updated_at = stamp();
      return snap(c);
    },
    async previewCohort(orgId, definition) {
      ensureOrg(orgId);
      const all = !definition.age_bands?.length;
      const bands = [
        { band: "24-35", n: 30, frames_n: 10 },
        { band: "36-47", n: 7, frames_n: 2 },
      ].filter((b) => all || definition.age_bands!.includes(b.band));
      const n = bands.reduce((s, b) => s + b.n, 0);
      return {
        n: n < SMALL_CELL_K ? null : n,
        by_age_band: bands.map((b) =>
          b.n < SMALL_CELL_K ? { band: b.band, n: null, frames_n: null, suppressed: true } : { ...b, suppressed: false },
        ),
        by_level: definition.require_frames ? { frames: 12, score: 0 } : { frames: 12, score: n - 12 },
        excluded: { no_consent: 14, withdrawn: 1, unscorable: 2, no_age: 2, quality: 1 },
        small_cell_k: SMALL_CELL_K,
      };
    },
    async listAnalysisRuns(orgId, q) {
      ensureOrg(orgId);
      const items = newest(runs).filter(
        (r) => (!q?.status || r.status === q.status) && (!q?.kind || r.kind === q.kind) && (!q?.experiment_id || r.experiment_id === q.experiment_id),
      );
      return page(items, q?.cursor, q?.limit);
    },
    async createAnalysisRun(orgId, req) {
      ensureOrg(orgId);
      const e = getExperiment(req.experiment_id);
      const active = [...runs.values()].filter((r) => r.status === "queued" || r.status === "running").length;
      if (active >= 5) throw problem(429, "analysis-quota-exceeded", "Quota exceeded", "too many queued runs");
      const r = run(newId("a"), {
        kind: req.kind, experiment_id: e.id, cohort_id: req.cohort_id ?? null, level_requested: req.level ?? "auto",
        seed: req.seed ?? 42, created_at: stamp(),
      });
      runs.set(r.id, r);
      e.last_run_id = r.id;
      return snap(r);
    },
    async getAnalysisRun(id) {
      const r = getRun(id);
      advance(r);
      return snap(r);
    },
    async listAnalysisRunSessions(id, q) {
      getRun(id);
      const all = rows.get(id) ?? [];
      const start = q?.cursor ? Number.parseInt(q.cursor, 10) || 0 : 0;
      const limit = q?.limit ?? 20;
      return { items: snap(all.slice(start, start + limit)), next_cursor: start + limit < all.length ? String(start + limit) : null };
    },
    async cancelAnalysisRun(id) {
      const r = getRun(id);
      if (r.status !== "queued" && r.status !== "running") throw problem(409, "job-finished", "Job finished", "the run already finished");
      Object.assign(r, { status: "cancelled", finished_at: stamp(), progress: null });
      return snap(r);
    },
    async rerunAnalysisRun(id) {
      const old = getRun(id);
      const r: AnalysisRun = {
        ...snap(old), id: newId("a"), status: "queued", summary: null, error: null, progress: null,
        result_bytes: null, started_at: null, finished_at: null, created_at: stamp(),
      };
      runs.set(r.id, r);
      return snap(r);
    },
    async getResearchFrames() {
      // The mock stores no research frame log: the preview shows the score-level result.
      throw problem(404, "frames-unavailable", "Not found", "no research frame log was uploaded for this session");
    },
    async getEffectiveConfig(orgId) {
      ensureOrg(orgId);
      return {
        source: "global",
        bundle_version: null,
        // The drift thresholds (INV-2) are what the session Quality tab draws as warn/hard lines.
        scoring_profile: { version: "1.1.0", body: { ...profile(5), trackingQuality: { drift: { warnDeg: 1.5, hardDeg: 3 } } } },
        normative_baselines: { version: "2026.10.0", body: baselineBody() },
        stimulus_catalog: { version: "1.4.0", body: {} },
      };
    },
  };
}
