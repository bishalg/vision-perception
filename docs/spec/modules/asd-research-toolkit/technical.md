# ASD researcher toolkit: parameter registry, tunable analysis, cohort runs - Technical Spec

Decision record: `docs/spec/adr/ADR-0012-researcher-toolkit-tunable-analysis.md`; platform
principles (no hard-coded parameters, every string localised, runtime tunability):
`docs/spec/adr/ADR-0013-three-audience-platform-architecture.md`. Work packages:
`work-packages.md` in this folder; acceptance scenarios: `acceptance.md`. Written
2026-11-03 against `dev` at `ee82015`.

Builds on `asd-rust-core-and-backend/technical.md` (CORE-*, CON-*, CFG-*, AUTH-*, ING-*),
`asd-gaze-robustness/technical.md` (PIPE-*, REP-*, SCH-3, QS-*, SCR-*),
`asd-clinician-and-interop/technical.md` (RBAC-*, KEY-*, EXJ-*, XC-*, CTR-*, UI-*) and
`asd-emr-lite-and-sharing/technical.md` (REC-6/7 for the sealed-blob pattern, HIP-1 for
read audits). Ids here: **PRM** (parameter registry), **INV** (constants inventory),
**RFL** (research frame log), **RPX** (reprocessing), **EXP** (experiments), **COH**
(cohorts), **RUN** (analysis runs), **SWP** (sweeps), **AGE** (age-driven analysis),
**PRV** (privacy), **GOV** (governance), **RUI** (console). Every MUST is testable; the
proving test or command is named in `work-packages.md` per package.

## Purpose and scope

In: a self-describing registry of every tunable knob; the scoring profile 1.2.0 that
holds the knobs that were constants; a consent-gated, server-readable research frame log;
reprocessing from raw samples in Rust (core, wasm, CLI); experiments (named draft
profiles), cohorts, analysis runs (cohort re-score, sweeps, age profiles) with
reproducibility records; small-cell suppression; research exports of run results; RBAC,
audit and resource limits; the console research area. Out: new metrics or new maths
(the toolkit tunes what exists), Android frame logging before an Android app exists
(the writer is Rust; the Kotlin shell comes with WP-M.*), publishing without the four-eyes
lifecycle, any Python.

## Verified current state this spec builds on

- **Profile and bundle.** `asd_contract::config::ScoringProfile` 1.1.0 = `flag`,
  `lowConfidenceValidRatio`, `qualityGate` (six limits), `trackingQuality`
  (`TrackingQualityParams` 1.0.0: `calibration`, `recalibration`, `validation`, `drift`,
  `trackState`, `confidence`, `gapFill`, `filter`, `verdict`, `scoring`; every struct
  `serde(default)`; `validate()` returns `ParamsProblem{path, message}`).
  `profile_checks::scoring_profile_problems` is shared by backend publish, wasm
  `validateScoringProfile` and `asd_sync::accept_bundle`. The device builds its pipeline
  configuration from the profile (`asd-ffi gaze_config_json_for_profile`). Config
  documents: `config_documents(kind, version UNIQUE, status draft|candidate|published|
  retired, body, created_by, approved_by, published_by …)`, global (no org), create /
  approve / publish / retire under `ConfigGlobalManage` (superadmin) with `four_eyes`;
  `ConfigWrite` (owner, admin, researcher) is unused. Bundles: `config_bundles` signed
  Ed25519, `org_id` NULL = global, `GET /v1/config/bundle` with ETag for devices.
- **Constants that are not in any document** (INV-1 lists them): `metrics.rs`
  `VELOCITY_THRESHOLD_UNITS_PER_SEC = 5.0`, `MIN_FIXATION_DURATION_SEC = 0.08`,
  `FRAME_INTERVAL_SEC = 1/60`, smile window `0.5` s, flat-affect gain `50`; `scoring.rs`
  `LOW_CONFIDENCE_VALID_RATIO = 0.60` (and `asd-wasm/src/api.rs:54` ignores the profile
  value on purpose); `norms.rs` `DEFAULT_MIN_N = 12`, `default_age_brackets()` (12-17,
  18-23, 24-35, 36-47, 48-60); `replay.rs` uses `QualityGate::default()`; `aoi_space.rs`
  has no margin; `ingest/auto_review.rs` `LOW_QUALITY_VALID_RATIO = 0.80`,
  `derive.rs` `MIN_VALID_RATIO_FOR_FLAG = 0.60`, `norm_aggregate.rs`
  `MIN_VALID_FRAME_RATIO = 0.80`.
- **Frames.** `sessions.frames_blob_key` points at `frames.jsonl.enc`, AES-GCM under the
  device Keychain key (`SessionVaultManager`), never served, purged by
  `jobs/retention.rs` after `organizations.retention_months`; `GET /v1/sessions/{id}/raw`
  returns the session document only (`frames_available`, `frames_purged`). Raw tracking
  logs (`asd-raw-tracking/1`, `RawLogHeader{format, source, deviceModel, orientation,
  geometrySource, geometry, targets[], appVersion, condition, groundTruth}` +
  `RawTrackingSample{t_sec, face_tracked, face_transform, look_at, left/right_eye_transform,
  eye_blink_left/right, ambient_*, light_direction, orientation, clip_media_time_sec,
  stimulus_id}`) are written by `BenchLogWriter` (Debug `ASD_BENCH_CAPTURE`, through
  `asd-ffi gaze_raw_log_header_line/gaze_raw_log_line`), `synth_tracking` and the Android
  converter. `gaze::replay(&RawLog, &ReplayOptions{params, model, glasses}) ->
  ReplayReport{frames, recorded_frames, targets, advice, summary, clips}`; replay
  produces no `active_aoi_names` (logs carry no AOIs) and no metrics.
  `pipeline::TelemetryWriter::sample(&ProcessedSample) -> GazeTelemetrySample`;
  `quality::evaluate_sample`; `metrics::extract_with_roles(&[GazeFrame], &AoiRole)`;
  `scoring::score_session_with(.., low_confidence_valid_ratio)`; `flag::assess`.
  `GazeFrame{gaze{valid, xNorm, yNorm, videoTimeSec, clipMediaTimeSec}, affect{3},
  activeAOINames, qualityVerdict, stimulusId}`.
- **Sessions.** Columns include `summary jsonb` (the whole `rawPayload`),
  `quality_verdict`, `valid_frame_ratio`, `tracking_verdict`, `tracking_accuracy_deg`,
  `scoring_status`, `attention_flag`, `device_id`, `bundle_version`, `received_at`,
  `frames_purged_at`; **no** `age_months` (only `summary->'ageMonths'`), no sex, no site;
  research consent only as `summary->'normCollection'->>'consented'`
  (`ingest::derive::norm_consented`). `session_clip_scores` holds the per-clip rows.
  `norm_aggregate` reads sessions in pages of 500 and withholds cells under `min_n`.
- **Jobs.** `jobs(kind text, status queued|running|succeeded|failed|cancelled, params,
  result, attempts, run_after, heartbeat_at, locked_by, org_id)`; `jobs::enqueue` inside
  the caller's transaction; phased handlers get `JobContext{pool, blobs, keyring, config,
  heartbeat_every}`; `MAX_ATTEMPTS = 3`, visibility 15 min, reclaim every 60 s; `GET
  /v1/jobs/{id}` only; no cancel, progress, quota or list.
- **Exports.** `ExportFormat` (12 values, `exports_format_check`), `ExportRequest{format,
  mode, purpose, filters, destination_ids, reason, include_labels, case}`, job phases
  snapshot → render → upload → record, `deid.rs` (`P-`/`S-` HMAC pseudonyms with a fresh
  salt, `YYYY-MM`), `dataset.rs` (JSONL + Parquet, `DatasetRow`), `tabular/csv.rs`,
  `MAX_EXPORT_ROWS = 200_000`, 500 MB. No small-cell rule anywhere.
- **RBAC/audit.** `auth/rbac.rs` `Action` (40), `MATRIX`, `allows`, `authorize`,
  `Require<perm::X>`; `superadmin` minus `CLINICAL_CONTENT_ACTIONS`; `researcher` holds
  `ConfigRead, ConfigWrite, JobRun, ExportRead, ExportDeidentified` plus the all-roles
  reads. `audit::AuditEvent::{user,device,system}(..).org().before().after().meta().record()`
  in the handler transaction; `audit::record_read` for reads (HIP-1). Names `noun.verb`.
- **Console.** `src/lib/asd/{browser,core,ops}.ts` load `asd-wasm` from `/asd-wasm/`;
  `SandboxRunner` + `useCoreCheck` (150 ms debounce); `DocVersionsPage`, `DraftShell`,
  `ScoringProfileEditor` (hand-written `NumberField` per field; the opposite of PRM-6);
  inline SVG charts `DriftChart`, `LossBar`, `Distribution`; `can(role, action)` in
  `lib/permissions.ts`, `navLinks` in `lib/nav.ts`; `RetentionPanel` is the only job
  poller; messages in `src/messages/*.ts` (English only); no literal-string lint.
- **iOS.** `NormCollectionGate.isActive` = research consent + `normCollectionOptIn` +
  TrueDepth; `QueuedSessionUpload.Stage{post, uploadBlobs}` with per-blob `done`;
  `BackgroundBlobUploader` with `Content-Digest` (SYNC-5/6); `ConfigBundleService`
  installs a verified bundle at next launch; `GazeTelemetryWriterHandle` (frames file).

## 1. Parameter registry (PRM) and the 1.2.0 profile

- **PRM-1** `asd_contract::params` defines `ParamSpec { id: String /* camelCase dotted
  wire path */, document: DocumentKind /* scoring_profile */, kind: ParamKind, default:
  Value, stage: Stage, reprocess: Reprocess, label_key: String, help_key: String, since:
  String /* profile schema version */, owner_gate: Option<String> }`, `ParamKind {
  Number { min, max, step, unit: Unit }, Integer { min, max, unit }, Bool, Enum { values:
  Vec<String> }, PointList { min_points, max_points } }`, `Unit { None, Ratio, Seconds,
  Millis, Cm, Mm, Deg, DegPerSec, Rad, Hz, NormUnits, NormUnitsPerSec, Months, Count }`,
  `Stage { Capture, Preprocess, Fixation, Metrics, Scoring, Flag, Norms, Display }`,
  `Reprocess { Score, Metrics, Frames, CaptureOnly }`. `pub fn registry() -> &'static
  [ParamSpec]` and `pub fn registry_version() -> &'static str` (`"1.0.0"`, bumped when
  a spec changes meaning, not when a default changes).
- **PRM-2** Completeness test: serialise `ScoringProfile::default()` to JSON, collect
  every leaf path (arrays of points count as one leaf), and assert the set equals the
  set of registry ids (both directions). Adding a field without a spec, or a spec
  without a field, fails `cargo test -p asd-contract`.
- **PRM-3** `params::validate(document: DocumentKind, body: &Value) -> Vec<ProfileProblem>`
  checks each registered id present in `body`: type, finite, `min ≤ v ≤ max`, enum
  membership, point ranges `[0,1]`; unknown keys under a registered object are a problem
  (`<path>: unknown parameter`), then appends `profile_checks::scoring_profile_problems`
  (cross-field rules and `TrackingQualityParams::validate`). It is the function the
  backend (`config_docs::lifecycle::validate_body`), wasm (`validateScoringProfile`) and
  `asd_sync::accept_bundle` call; none of them keeps a private range check.
- **PRM-4** Schema export: `asd-cli schema export` writes
  `docs/spec/schemas/asd/param-registry-v1.json` (`{registryVersion, params: [ParamSpec]}`,
  `$id https://schemas.curv.org/asd/param-registry-v1.json`) and `schema check` fails on
  drift; `asd-cli params list [--stage S] [--reprocess R]` prints a table; `asd-cli params
  check FILE` validates a profile JSON.
- **PRM-5** Message keys: `label_key = "param.<id>.label"`, `help_key = "param.<id>.help"`;
  the English and Nepali texts live in the shared domain catalogue (ADR-0013 principle 2,
  WP-L.0) and never in the registry; a test asserts every key has both locales.
- **PRM-6** Generic UI: the console (RUI-2), iOS and Android render a tuning form from
  the registry: one component per `ParamKind`, grouped by the first path segment, in
  registry order, with the unit label, the published value, the draft value, the
  default, and the `reprocess` badge; `capture_only` knobs are rendered disabled in a
  preview context. No per-knob component exists; a new knob is a registry row plus two
  message keys.
- **PRM-7** `ScoringProfile` schema **1.2.0** adds, all `serde(default)` to today's
  constants: `metrics: MetricsParams { ivt_velocity_units_per_sec: 5.0, ivt_velocity_deg_per_sec:
  30.0 (used when `ivt_unit = "deg"`), ivt_unit: "norm" | "deg" (default "norm"; "deg" is
  SCR-3, owner_gate H-G2), min_fixation_duration_sec: 0.08, fallback_frame_interval_sec:
  1/60, smile_reactivity_window_sec: 0.5, flat_affect_gain: 50.0 }`, `aoi: AoiParams {
  margin_norm: 0.0 (−0.1..0.1; negative shrinks), dwell_cap_ms: 100 }`, `age_bands:
  Vec<AgeBracket> (default the five brackets; contiguous, ascending, 1..=12 bands,
  months 0..240)`, `norms: NormsParams { min_n: 12 (5..100000), include_imputed: false
  }`. `SCORING_PROFILE_SCHEMA_VERSION = "1.2.0"`; a 1.1.0 or 1.0.0 body decodes to the
  defaults; `config-bundle-v1.json` and `api-v1.json` regenerate. The existing
  `trackingQuality.scoring.*` keep their meaning (SCR-1/2 behind the owner gate) and are
  registered with `owner_gate: Some("H-G2")`.
- **PRM-8** `asd_contract::config::GazeProcessingView<'a> { quality_gate, tracking_quality,
  metrics, aoi }` borrows the four sections for callers that only process gaze
  (`gaze::reprocess`, `gaze_config_json_for_profile`); it is a type, not a document.
- **PRM-9** `asd-core` reads every INV-1 value from params: `metrics::extract_with(frames,
  &AoiRole, &MetricsParams, &AoiParams)` (`extract`/`extract_with_roles` keep their
  signatures and call it with defaults), `scoring::score_session` calls
  `score_session_with(.., LOW_CONFIDENCE_VALID_RATIO)` (unchanged) and every product
  caller (wasm, ffi, backend) passes the profile value, `norms::AggregateParams` is built
  from `NormsParams` + `age_bands`, `replay()` gains `ReplayOptions.quality_gate:
  Option<QualityGate>`, `aoi_space::clip_aois_with_margin(.., margin_norm)`. Golden
  vectors (`asd-core/tests/golden*`, kit `GoldenParityTests`, Android
  `GoldenParityTest.kt`) stay byte-identical with the default profile.

## 2. Constants inventory and migration (INV)

- **INV-1** The following constants move into the registry in phase 8 (WP-X.1 core,
  WP-X.2 backend) or are recorded as deliberate non-parameters:

  | Where | Constant | Value | Registry id (stage/reprocess) or verdict |
  | --- | --- | --- | --- |
  | `asd-core/metrics.rs:25` | `VELOCITY_THRESHOLD_UNITS_PER_SEC` | 5.0 | `metrics.ivtVelocityUnitsPerSec` (fixation/frames) |
  | `metrics.rs:27` | `MIN_FIXATION_DURATION_SEC` | 0.08 | `metrics.minFixationDurationSec` (fixation/frames) |
  | `metrics.rs:29` | `FRAME_INTERVAL_SEC` | 1/60 | `metrics.fallbackFrameIntervalSec` (metrics/frames) |
  | `metrics.rs` inline | smile window / flat-affect gain | 0.5 s / 50 | `metrics.smileReactivityWindowSec`, `metrics.flatAffectGain` (metrics/frames) |
  | `scoring.rs:25` | `LOW_CONFIDENCE_VALID_RATIO` | 0.60 | existing `lowConfidenceValidRatio`, now honoured by wasm/ffi/backend (scoring/score) |
  | `aoi_space.rs` | hit test inclusive, no margin | 0 | `aoi.marginNorm` (metrics/frames) |
  | kit `GazeProcessingActor.swift:1011` | `maxFrameDwellSec` | 0.1 | `aoi.dwellCapMs` (metrics/frames) |
  | `norms.rs:24`, `norm_aggregate.rs` | `DEFAULT_MIN_N`, `min_n` | 12 | `norms.minN` (norms/score) |
  | `norms.rs`, `synth.rs`, iOS `ChildProfile.swift:45`, console `participant.ts:64` | age brackets | 5 bands | `ageBands` (norms/score); iOS and console labels come from the published profile, not literals |
  | `trackingQuality.scoring.ivtVelocityDegPerSec` | | 30 | kept; `metrics.ivtUnit = deg` selects it (owner_gate H-G2) |
  | backend `auto_review.rs:31` | `LOW_QUALITY_VALID_RATIO` | 0.80 | `review.lowQualityValidRatio` (display/score) — org-overridable rule, same profile |
  | backend `derive.rs:18` | `MIN_VALID_RATIO_FOR_FLAG` | 0.60 | = `lowConfidenceValidRatio` (read from the published profile) |
  | backend `norm_aggregate.rs:48` | `MIN_VALID_FRAME_RATIO` | 0.80 | = `review.lowQualityValidRatio` |
  | iOS result views (`PerVideoResultViews.swift:584`, `DoctorReportGenerator.swift:318`, `EngagementRiskStratificationView.swift`, `EngagementDetailViews.swift`, `EngagementAffectAndQualityViews.swift`) | traffic-light cut-offs | 0.65/0.35, 60/50 %, 550/750 ms, 20/10 %, 75 % | `flag.*` already delivered; the per-metric display bands become `display.bands.<metric>` (display/score), rendered from the bundle (WP-M.1) |
  | console `DriftChart` | warn/hard lines not drawn | | read `trackingQuality.drift.warnDeg/hardDeg` from the published profile (WP-X.7 passes them) |
  | `raycast.rs`, `calibration.rs`, `pipeline.rs` `EPS_*`, `RANK_TOL`, `RAY_T_*`, `GRAZING_MIN_ANGLE_DEG`, `mediapipe.rs` camera model | numerical guards and physical model constants | | **not parameters**: documented as such in `gaze/mod.rs`; a change is a `PIPELINE_VERSION` bump |
  | job/auth/share/retention limits (runner attempts, TTLs, share-link bounds, housekeeping days) | | | **operational settings**, not analysis knobs: stay constants or env (`ASD_*`), listed in `rust/README.md`; `ASD_ANALYSIS_*` added by RUN-9 |

- **INV-2** A registry entry with `stage = Display` is rendered by the console and the
  apps from the **published** profile (`GET /v1/orgs/{id}/config/effective`, GOV-6) and
  by the device from its bundle; no surface keeps a copy of a cut-off.

## 3. Research frame log (RFL)

- **RFL-1** Format `asd-raw-tracking/1` is extended additively (asd-core `raw_log.rs`,
  `types/tracking.rs`): `RawLogHeader` gains `clips: Vec<RawLogClip { clip_id, order,
  catalog_version, aois: Vec<ScreenAoi> /* the screen-space AOIs the device used,
  after aspect-fill mapping */, video_aspect }>`, `bundle_version: Option<String>`,
  `profile_version: Option<String>`, `session_id: Option<String>`, `participant_token:
  Option<String>` (never in the de-identified export; see PRV-5); `RawTrackingSample`
  gains `affect: Option<GazeFrameAffect>` (three numbers, present on recorded frames).
  All new fields `serde(default)`; `raw_log::parse` accepts old logs; `replay` ignores
  the additions. `asd-ffi` gains `gaze_research_log_header_line(header_json)` and
  `gaze_research_log_sample_line(sample_json)` (thin wrappers; the kit never formats a
  line itself).
- **RFL-2** iOS: when `NormCollectionGate.isActive` for the session, `GazeProcessingActor`
  writes `research.jsonl` next to the vault file (plain, `completeFileProtection`,
  excluded from backup) from the same `RawTrackingSample` it pushes into the pipeline,
  plus the affect of that frame; the header is written at session end (clips and AOIs
  known). Size cap 16 MiB (a session over the cap drops the log and records
  `research_log_dropped: true` in `normCollection`). The queue gains blob kind
  `research_frames` in `uploadBlobs` (same `BackgroundBlobUploader`, `Content-Digest`
  required); the file is deleted after a successful upload or with the session.
- **RFL-3** Backend: `BlobKind::ResearchFrames`, key
  `sessions/{org}/{token}/{session}/research.jsonl.sealed`, accepted **only** through the
  proxy route `PUT /v1/sessions/{id}/blobs/research_frames` (device bearer; the ack
  carries `research_frames_upload_url` only in proxy mode; presigned mode never offers
  it, because the bytes must pass through the encryptor). Rules: `Content-Digest`
  required (`400 digest-required`), ≤ 16 MiB, first line parses as a `RawLogHeader`
  with `format = asd-raw-tracking/1` and `session_id` equal to the path, the session's
  `summary.normCollection.consented` is true (else `409 research-consent-required`), the
  participant is not withdrawn. The body is sealed under the org DEK (KEY-1, AAD
  `sessions:research_frames:{org}:{session_id}`) before `BlobStore::put`;
  `sessions.research_frames_blob_key`, `research_frames_bytes`, `research_frames_sha256`
  are set; audit `session.frames_upload` (device actor, bytes, sha256). Re-PUT replaces
  (same rules as frames).
- **RFL-4** `GET /v1/sessions/{id}/research-frames` (`FramesRead`: owner, researcher;
  `superadmin` denied): streams the decrypted log (`application/x-ndjson`,
  `Cache-Control: no-store`), `404 frames-not-available` when never uploaded, `410
  frames-purged` after the sweep; audits `session.frames_read` (HIP-1 style, fail
  closed).
- **RFL-5** Retention: `organizations.research_frames_retention_months int NULL CHECK
  (1..240)`; the nightly sweep purges research logs of sessions received before
  `now() - coalesce(research_frames_retention_months, retention_months)` months,
  clears the key and sets `research_frames_purged_at`; `RetentionOrgResult` gains
  `research_frames_purged`. Erasure (EXJ-8) deletes the blob with the session's others.
- **RFL-6** `SessionRaw` and `GET /v1/sessions/{id}` gain `research_frames: { available:
  bool, purged: bool, bytes? }`; the session list filter `?frames=available` and the
  cohort preview use `research_frames_blob_key IS NOT NULL`.
- **RFL-7** The device-sealed `frames.jsonl.enc` path is unchanged by this spec; O-13
  (stop uploading it) is ADR-0012 open question 4 and, when decided, a one-line iOS
  change plus the sweep.
- **RFL-8** Logs are biometric-adjacent numbers (ADR-0007 D13): never in git, never in a
  de-identified export (PRV-5), never readable by the vendor role, retention-bound.

## 4. Reprocessing (RPX)

- **RPX-1** `asd_core::gaze::reprocess::reprocess(log: &RawLog, catalog: &StimulusCatalog,
  view: &GazeProcessingView, options: &ReprocessOptions { model: Option<CalModelKind>,
  glasses: Option<bool>, trace_hz: Option<u32> /* down-sampled trace for plotting */ })
  -> Result<Reprocessed, ReprocessError>` runs `replay` with the view's gate and params,
  maps each clip's AOIs from the log header (`header.clips[].aois`; when absent, from
  the catalog version named in the header through `clip_aois_with_margin`), feeds
  processed samples of recorded frames through `TelemetryWriter` + `evaluate_sample` to
  build `GazeFrame`s with `active_aoi_names` (point-in-AOI with `aoi.margin_norm`) and
  the affect carried on the sample, then `metrics::extract_with`. `Reprocessed {
  metric_sets, frames_n, valid_ratio, quality_verdicts, tracking: TrackingQuality,
  trace: Option<Vec<TracePoint { t_sec, x, y, state, aoi, fixation_id }>>, fixations:
  Vec<Fixation> }`.
- **RPX-2** `asd_core::analysis::evaluate(session: &SessionInput, profile: &ScoringProfile,
  baselines: &NormativeBaselineSet, level: Level) -> Evaluation` is the one function
  every caller (wasm preview, backend job, CLI) uses: `Level::Score` rebuilds metric
  sets from `perVideo` rows (the existing `asd-wasm::session::metric_sets_from_reports`
  moves into core as `metrics::from_reports`), `Level::Frames` calls RPX-1; both then
  `score_session_with(.., profile.low_confidence_valid_ratio)`, `flag::assess(..,
  &profile.flag)`, and return `Evaluation { level, score: PerVideoSessionScore, flag:
  AttentionFlagAssessment, quality_status: QualityStatus /* from valid ratio and
  tracking verdict with the profile's limits */, metrics: Vec<PerVideoMetricSet>, tracking:
  Option<TrackingQuality>, warnings }`. `ageMonths` comes from the session
  (`summary.ageMonths`); a session without it is `unscorable: no_age` at every level.
- **RPX-3** Level resolution (`Level::Auto`): `Frames` when the session has a research
  log (and, in the browser, when the caller fetched it), else `Score`; a knob set whose
  registry `reprocess` is `Frames` or `Metrics` applied at `Score` level marks the result
  `warnings: ["param_not_applied: <id>"]` and the run counts it (RUN-4); `capture_only`
  knobs are never applied and are reported the same way.
- **RPX-4** `asd-wasm` exports `paramRegistry() -> {registryVersion, params}`,
  `reprocessSession(frameLogJsonl: string, catalog, profile, baselines, options {
  ageMonths, scoredAtUtc?, model?, glasses?, traceHz? }) -> Evaluation + trace`, and
  `rescoreSession` gains the `metrics`/`lowConfidenceValidRatio` behaviour of RPX-2
  (`metric_source: session_report`, level `score`). Parity test: native and wasm32 give
  byte-identical JSON for the synth scenarios through both levels.
- **RPX-5** `asd-cli research reprocess LOG --catalog FILE --profile FILE --baselines FILE
  --age-months N [--level auto|score|frames] [--trace-hz 10]` prints the `Evaluation`;
  `asd-cli research evaluate SESSION.json …` runs `Level::Score`. Both exit 0/1/2 like
  `gaze replay`.
- **RPX-6** Determinism: `reprocess` and `evaluate` are pure; the same inputs give the
  same bytes on every platform (tested on the synth scenarios: iOS pipeline output of a
  scripted log, WP-G.7 parity, equals `reprocess`'s frames for the recorded range).

## 5. Experiments (EXP), cohorts (COH), analysis runs (RUN), sweeps (SWP), age (AGE)

- **EXP-1** `experiments(id uuid PK, org_id, name text (1..80), description text (≤ 500),
  status text CHECK IN ('draft','archived','promoted') DEFAULT 'draft', base_profile_version
  text, profile jsonb /* full ScoringProfile 1.2.0 */, profile_hash bytea /* sha256 JCS */,
  baselines_version text NULL /* published or draft document version to score against */,
  promoted_config_version text NULL, last_run_id uuid NULL, row_version int DEFAULT 1,
  created_by uuid, created_at, updated_at, UNIQUE (org_id, name))`. Migration **0026**.
- **EXP-2** Routes: `POST /v1/orgs/{id}/experiments` (`ExperimentWrite`; body
  `CreateExperimentRequest { name, description?, from_experiment_id? | from_version? }`,
  starts from the published profile by default; `409 experiment-name-exists`), `GET
  /v1/orgs/{id}/experiments?status&cursor&limit` (`AnalysisRead`), `GET
  /v1/experiments/{id}`, `PATCH /v1/experiments/{id}` (`If-Match: "<row_version>"`, `412
  stale-version`; body `{name?, description?, profile?, baselines_version?}`; the
  profile is validated by PRM-3, `400 config-invalid` with `field_problems`), `POST
  /v1/experiments/{id}/archive`, `POST /v1/experiments/{id}/promote` (GOV-2). Audits
  `experiment.create|update|archive|promote` with ids, `profile_hash` and the changed
  registry ids (never values of clinical text; there is none).
- **COH-1** `CohortDefinition { age_bands: Vec<String> /* ids from the profile's
  ageBands; empty = all */, sex: Vec<Sex> /* applied only if the session carries sex;
  empty = all */, device_ids: Vec<Uuid> /* "site"; empty = all */, from: Option<date>,
  to: Option<date> /* received_at */, quality_verdicts: Vec<GazeQualityVerdict>,
  tracking_verdicts: Vec<TrackingVerdict>, min_valid_ratio: Option<f64>, scoring_status:
  Vec<ScoringStatus> /* default scored */, require_frames: bool, capture_source: real
  (fixed) }`. Research consent (`summary.normCollection.consented = true`) and `not
  withdrawn` are always applied and not expressible otherwise; synthetic sessions are
  included only with `ASD_ALLOW_SYNTHETIC` and `include_synthetic`, flagged in every
  result. `cohort_hash = sha256(JCS(definition))`.
- **COH-2** `cohorts(id, org_id, name (1..80), definition jsonb, definition_hash bytea,
  created_by, created_at, updated_at, archived_at NULL, UNIQUE (org_id, name))`; routes
  `POST/GET /v1/orgs/{id}/cohorts`, `GET/PATCH /v1/cohorts/{id}`, `POST
  /v1/orgs/{id}/cohorts/preview` (`AnalysisRead`; body = definition; returns `{ n,
  by_age_band: [{band, n, frames_n}], by_level: {frames, score}, excluded:
  {no_consent, withdrawn, unscorable, no_age, quality} }` with PRV-1 suppression; no
  job, one SQL). Audits `cohort.create|update|archive`.
- **COH-3** `sessions.age_months int NULL CHECK (0..240)` is added and back-filled from
  `summary->>'ageMonths'` in 0026; ingest sets it; cohort band filters use it with the
  profile's `ageBands` (band edges evaluated at run time from the experiment's profile,
  so a researcher may test other bands; the stored summary names the bands used).
- **RUN-1** `analysis_runs(id uuid PK, org_id, kind text CHECK IN ('cohort_rescore',
  'param_sweep','age_profile'), status text CHECK IN ('queued','running','succeeded',
  'failed','cancelled'), experiment_id uuid NULL REFERENCES experiments, cohort_id uuid
  NULL REFERENCES cohorts, definition jsonb /* AnalysisDefinition snapshot: cohort,
  profile, baselines, sweep, level, options */, definition_hash bytea, cohort_hash bytea,
  membership_hash bytea NULL, profile_hash bytea, published_profile_version text,
  baselines_version text, catalog_version text, bundle_version text NULL, core_version
  text, contract_version text, pipeline_version text, registry_version text, seed bigint,
  level_requested text, sessions_n int, sessions_frames_n int, sessions_score_n int,
  excluded jsonb, summary jsonb, result_blob_key text NULL, result_bytes int NULL,
  progress jsonb NULL, job_id uuid REFERENCES jobs, error text NULL, created_by,
  created_at, started_at, finished_at)`; index `(org_id, created_at DESC, id DESC)`.
- **RUN-2** `POST /v1/orgs/{id}/analysis-runs` (`AnalysisRun`; body `AnalysisRunRequest {
  kind, experiment_id, cohort_id? | cohort?: CohortDefinition, sweep?: SweepSpec, level:
  auto|score|frames, seed?, options { metrics: Vec<String> /* metric keys to summarise;
  default the 15 keys + composite */, bootstrap: bool /* CI, default true */ } }`)
  validates (PRM-3 on the experiment profile at that moment, SWP-2 budget, RUN-9 quota),
  snapshots the definition, inserts the run and enqueues job kind `analysis_run` in the
  same transaction, audits `analysis.run_enqueue`, answers `202 AnalysisRun`. `GET
  /v1/orgs/{id}/analysis-runs?kind&status&experiment_id&cursor`, `GET
  /v1/analysis-runs/{id}` (`AnalysisRead`; audits `analysis.read` once per call), `GET
  /v1/analysis-runs/{id}/sessions?cursor&limit` (`AnalysisRead` + `SessionRead`; rows
  from the sealed blob, real session ids, level, old/new composite, flag, quality status,
  metric values; audited once per page as `analysis.sessions_read`), `POST
  /v1/analysis-runs/{id}/cancel` (`AnalysisRun`; sets `jobs.cancel_requested_at`, audits
  `analysis.run_cancel`), `POST /v1/analysis-runs/{id}/rerun` (`AnalysisRun`; new run from
  the stored definition; the response includes `membership_drift {added, removed}` once
  it finishes).
- **RUN-3** Job `analysis_run` (`jobs/analysis.rs`, phased with `JobContext`): (1) resolve
  membership with the cohort SQL inside a `REPEATABLE READ READ ONLY` snapshot, store
  `membership_hash` and counts; (2) stream sessions in pages of 200: for each, pick the
  level (RPX-3), fetch and unseal the research log when `Frames`, call
  `analysis::evaluate` twice (published profile + published baselines, experiment profile +
  its baselines), append one row to the result JSONL buffer, update the incremental
  aggregates; heartbeat every page; check `cancel_requested_at` every page; (3) seal
  the rows under the org DEK and `put_object_large` to `analysis/{org}/{run}/sessions.jsonl.sealed`
  (≤ 64 MiB, else `failed: result-too-large`); (4) write `summary`, counts, `finished_at`,
  audit `analysis.run_complete` (system; counts, no values). Per-session rows: `{
  session_id, level, age_months, age_band, device_id, published: {composite, flag,
  quality_status, metrics{..}}, draft: {…}, delta: {composite, metrics{..}}, warnings[] }`.
- **RUN-4** `summary` (JSONB, ≤ 1 MiB, PRV-1 applied): `{ counts: { sessions, by_level,
  excluded{reason: n}, params_not_applied{id: n} }, flags: { published: {green,yellow,red,
  unreliable}, draft: {…}, transitions: [[from,to,n]] }, quality: { transitions
  [[from,to,n]] }, by_age_band: [{ band, n, suppressed, metrics: { <key>: { published:
  Dist, draft: Dist, delta: Dist } }, composite: {published: Dist, draft: Dist, delta:
  Dist}, flag_rate: {published, draft} }], overall: {same shape} }` with `Dist { n, mean,
  sd, median, iqr: [q1,q3], p10, p90, ci95_mean: [lo,hi] (bootstrap, seed), hist: { edges,
  counts } }` (fields null when PRV-1 suppresses them).
- **RUN-5** Reproducibility: every column of RUN-1 from `definition_hash` to `seed` is
  set before the job starts (`core_version` = `asd-core` crate version, `pipeline_version`
  = `gaze::PIPELINE_VERSION`, `contract_version` = `api_v1` version, `registry_version`,
  `catalog_version` = the catalog the sessions reference, with the published catalog as
  fallback). `GET /analysis-runs/{id}` returns them all. Two runs with equal
  `definition_hash`, `membership_hash`, versions and `seed` produce byte-identical
  `summary` (test on synth data).
- **RUN-6** Re-open: the run page renders from `summary` and the rows endpoint; nothing is
  recomputed. `rerun` shows drift (sessions added or removed since) and version changes
  side by side.
- **RUN-7** Export: `ExportFormat::analysis_results` (`exports_format_check` widened in
  0026), `ExportRequest.analysis: Option<AnalysisExport { run_id, layout: aggregates |
  sessions, encoding: jsonl | csv | parquet /* parquet only for sessions */ }>`, mode
  de-identified only, purpose `research` or `quality_improvement`, `ExportDeidentified`;
  the job (`export_build/analysis.rs` → `asd_export::analysis`) writes `aggregates` as
  one JSONL/CSV table (`run_id, band, metric, side, n, mean, sd, median, q1, q3, p10, p90,
  ci_lo, ci_hi, suppressed`) and `sessions` through `deid.rs` (fresh salt, `S-`/`P-`
  pseudonyms, age → band, `session_month`) into the dataset encoder; the manifest's
  `provenance` block carries every RUN-5 field. Research export policy (XC-5) applies:
  only research-consented sessions ever enter a run (COH-1), so nothing is excluded here.
- **RUN-8** Progress and cancel for every job: `jobs.progress jsonb NULL` (`{done, total,
  phase}`) and `jobs.cancel_requested_at timestamptz NULL` (0026); `JobContext::progress(done,
  total, phase)` writes it with the heartbeat; `JobContext::cancelled()` reads the flag;
  a cancelled job ends `cancelled` with partial results discarded; `POST /v1/jobs/{id}/cancel`
  (`JobRun`, same org, `409 job-finished`); `GET /v1/jobs/{id}` returns `progress`.
- **RUN-9** Resource limits (env with defaults, listed in `rust/README.md`):
  `ASD_ANALYSIS_MAX_RUNNING_PER_ORG = 1`, `ASD_ANALYSIS_MAX_QUEUED_PER_ORG = 5` (`429
  analysis-quota-exceeded`), `ASD_ANALYSIS_MAX_SESSIONS = 2000` (score) /
  `ASD_ANALYSIS_MAX_FRAME_SESSIONS = 500` (frames) per run, `ASD_ANALYSIS_MAX_EVALUATIONS
  = 200000` (score) / `20000` (frames) including sweep cells, `ASD_ANALYSIS_MAX_MINUTES =
  30` (the job fails `budget_exceeded` at the deadline), result blob ≤ 64 MiB, one
  `analysis_run` at a time per worker (the worker skips the kind while one is running in
  that process). Preview counts (COH-2) show the budget before a run is queued; a
  request over budget is `400 sweep-too-large` or `400 cohort-too-large`.
- **SWP-1** `SweepSpec { axes: Vec<SweepAxis { param_id, from, to, step }> (1..=2), fixed:
  experiment profile }`; `param_id` must be a registry id of kind Number/Integer whose
  `reprocess` is allowed at the requested level; `from ≤ to`, `(to-from)/step ≤ 24`,
  values inside the registry range. Phase B (WP-X.9).
- **SWP-2** Budget: `cells = Π(steps+1) ≤ 625`; `cells × sessions ≤ ASD_ANALYSIS_MAX_EVALUATIONS`
  for the level; refused before queueing. The published value is always added as a cell
  when it falls off the grid.
- **SWP-3** Result `summary.sweep: { axes, cells: [{ values: [a, b?], n, flag_rate:
  {green,yellow,red,unreliable}, quality_rate: {..}, metrics: { <key>: {mean, median} },
  composite: {mean, median}, changed_vs_published: n }] }` plus the RUN-4 blocks for the
  published cell; the per-session blob stores only the published cell and the cell
  nearest the experiment's values (not all cells).
- **SWP-4** The browser can sweep one session (RUI-4) by calling `reprocessSession`
  per step; no server job is involved and the same `SweepSpec` shape is used.
- **AGE-1** `age_profile` runs (Phase B) compute, per band and metric, the cohort's
  `Dist` against the baseline's `NormativeRange` (`expected`, `acceptable_low/high`
  resolved by band): share on-target / monitor / atypical, the percentile of the
  expected value within the cohort, and the same for a draft baselines version when
  given ("how a draft baseline would move them"); the band edges come from the
  experiment's `ageBands`.
- **AGE-2** Bands are data: the console labels them from the profile (`12–17 mo` is
  formatted, not stored), the iOS `PediatricAgeBracket` raw values stop being labels
  (WP-M.1), `norms::aggregate` takes the profile's bands, and the norm job (WP-B.5) reads
  `ageBands` and `norms.minN` from the published profile instead of its constants.
- **AGE-3** Minimum-n rule: a band with `n < norms.minN` is reported as `suppressed` in
  every age table (PRV-1), and `cohorts/preview` warns before a run which bands will be
  suppressed.

## 6. Privacy (PRV)

- **PRV-1** Small-cell suppression, one function `asd_core::analysis::suppress(summary,
  k)`: every `Dist` with `n < k` → `{n: null, suppressed: true}` and no other field;
  `p10`/`p90` null when `n < 20`; transition-matrix cells with `0 < n < k` → `null`
  (zero stays zero; a `suppressed_cells: n` count is kept); flag rates for a band with
  `n < k` are null. `k = organizations.settings.research.small_cell_k` (default 12,
  bounded 5..50, `PATCH /orgs/{id}` validates). Applied by the job before `summary` is
  stored (so re-opening never shows more), by `cohorts/preview`, and by the export.
- **PRV-2** Per-session rows (`/sessions` endpoint) are visible only to roles that hold
  `SessionRead` for that org, which every org role does today; `superadmin` reads them
  through the same rule (no clinical content inside). They carry no token, MRN, name or
  device model, only `device_id` (for the site filter) and `age_months`.
- **PRV-3** Exports of rows (RUN-7 `sessions`) go through `deid.rs`: `S-`/`P-`
  pseudonyms with a fresh salt, `age_months → age_band`, `device_id` dropped (site
  becomes `site_pseudonym` through the same HMAC), dates `YYYY-MM`; `aggregates` carry
  no ids. The de-identified zip-scan test of WP-I.12 (no token, session id, MRN, device
  id) runs on both layouts.
- **PRV-4** Research consent is the only consent that admits a session to a run
  (`summary.normCollection.consented`), the conservative reading of ADR-0009 Q4; a
  withdrawn participant's sessions leave membership at the next run and their research
  log is deleted by erasure.
- **PRV-5** The research log is never exported in any format (it holds the participant
  token in its header and raw biometrics); `asd-export` has no encoder for it and a test
  asserts `ExportFormat` has no variant that reads `research_frames_blob_key`.
- **PRV-6** Claims gate: every research page and export README carries `This is not a
  diagnostic tool.`; the words used are "flag", "metric", "distribution", "status";
  `scripts/check_asd_claims.py --prose` (or its `asd-cli` port) covers
  `apps/curv_console/src/messages/*` from WP-X.5 on.

## 7. Governance (GOV)

- **GOV-1** RBAC additions (one matrix change): `AnalysisRead` (owner, admin, clinician,
  researcher, superadmin), `AnalysisRun` (owner, researcher), `ExperimentWrite` (owner,
  researcher), `ExperimentPromote` (owner, researcher), `FramesRead` (owner,
  researcher). `AnalysisRun`, `ExperimentWrite`, `ExperimentPromote` and `FramesRead`
  join the superadmin exclusion list (the vendor neither spends clinic CPU nor reads
  frames). Console `permissions.ts` mirrors the rows.
- **GOV-2** Promote: `POST /v1/experiments/{id}/promote { kind: scoring_profile |
  normative_baselines, version, notes }` creates a **draft** config document through
  `config_docs::lifecycle::create` with the experiment's body, `notes` prefixed by
  `experiment:<id> run:<last_run_id>` and `created_by` = the caller; needs
  `ExperimentPromote` and the route `POST /v1/config/{kind}` starts accepting
  `ConfigWrite` for the draft step (approve, publish and retire stay
  `ConfigGlobalManage`, four-eyes unchanged). The experiment becomes `promoted` with
  `promoted_config_version`; audits `experiment.promote` and `config.document.create`.
  `409 experiment-promoted` on a second attempt; `409 experiment-no-run` when
  `last_run_id` is null (an experiment must have been run on a cohort before it can be
  proposed; the run can be small).
- **GOV-3** Evidence before publish (server rule replacing the console's localStorage
  rule): `lifecycle::publish` of a `scoring_profile` or `normative_baselines` candidate
  requires a `succeeded` `analysis_run` whose `profile_hash` (or `baselines_version`)
  equals the candidate's body hash, else `409 config-unanalysed` (open question 3 may
  downgrade this to a warning field on the candidate). Seeded and vendor-authored
  documents are exempt when `created_by IS NULL`.
- **GOV-4** Audit names (AUTH-5 style): `experiment.create`, `experiment.update`,
  `experiment.archive`, `experiment.promote`, `cohort.create`, `cohort.update`,
  `cohort.archive`, `analysis.run_enqueue`, `analysis.run_complete` (system),
  `analysis.run_cancel`, `analysis.read`, `analysis.sessions_read`, `session.frames_upload`
  (device), `session.frames_read`, `export.create` (existing, format
  `analysis_results`). `after` carries ids, hashes, counts and changed registry ids; never
  per-session values.
- **GOV-5** Nothing in the toolkit writes to `sessions`, `session_clip_scores`,
  `review_items`, `clinical_*`, `config_documents` (except GOV-2's draft) or
  `config_bundles`; a test asserts the analysis job's role (`asd_app`) performs only
  `SELECT` on those tables (statement log in the test).
- **GOV-6** `GET /v1/orgs/{id}/config/effective` (`ConfigRead`, every org role) returns
  the published `scoring_profile`, `normative_baselines` and `stimulus_catalog` versions
  and bodies (the org bundle when one exists, else global) so the console renders
  display bands, drift lines, age-band labels and review thresholds from one source
  (INV-2).

## 8. Console (RUI)

- **RUI-1** Routes under `/[org]/research` (nav entry "Research" for `analysis.read`):
  `/research` (overview: recent runs, experiments, cohorts), `/research/experiments`,
  `/research/experiments/new`, `/research/experiments/[id]` (workbench),
  `/research/cohorts`, `/research/cohorts/new`, `/research/cohorts/[id]`,
  `/research/runs`, `/research/runs/[id]`, `/research/runs/[id]/sessions`. The existing
  `/config/sandbox` stays and links to the workbench; `/config/scoring/new` keeps its
  editor until WP-X.5 replaces `ScoringProfileEditor`'s hand-written fields with the
  registry form (`RegistryForm`), after which both pages share it.
- **RUI-2** `RegistryForm` (client): takes `{registry, published, draft, context:
  "preview" | "edit"}`, renders groups in registry order, one `ParamControl` per kind
  (`Number`/`Integer`: a native `type="range"` + `type="number"` pair with unit, min,
  max, step, default marker and a "reset" link; `Bool`: checkbox; `Enum`: select;
  `PointList`: the existing `AoiFrame`-style editor reused for normalised points),
  the `reprocess` badge (`score` / `frames` / `capture only`) and the diff count. Every
  label and help text is `t(param.<id>.label)` from the messages catalogue; a missing key
  renders the id and fails `npm test` (`messages.test.ts` asserts coverage of the
  registry).
- **RUI-3** Live preview panel: a session picker (recent 50 research-consented
  sessions with a "frames" badge from RFL-6; multi-select ≤ 10), the chosen level, and
  per session a card with published vs draft: composite, flag, quality status, per-clip
  subscores (`ClipsTable` reuse), AOI dwell bars (`Distribution` reuse), and at frames
  level a `ScanpathPlot` (inline SVG: down-sampled trace with fixation circles and AOI
  rectangles for one chosen clip, published and draft overlaid) and the tracking
  summary. Re-runs on every knob change with a 150 ms debounce in a `useTransition`; the
  research log is fetched once per session through a server action
  (`loadResearchFramesAction`, which calls RFL-4, so the audit row exists) and held in
  memory only.
- **RUI-4** Single-session sweep: pick one Number knob, from/to/step (≤ 24 steps); the
  panel calls `reprocessSession`/`rescoreSession` per step and draws `SweepCurve` (SVG
  line: composite and each selected metric against the knob, published value marked).
- **RUI-5** "Run on cohort" dialog: cohort picker (saved or inline definition with the
  COH-2 preview counts, suppressed bands flagged, budget line "n sessions × m cells ≤
  budget"), level, metrics to summarise, seed (optional), submit → `202` → the run page.
- **RUI-6** Run page: status with progress (`useJobPoll(jobId)` hook, 2 s, stops on a
  final state; extracted from `RetentionPanel` so both use it), then: counts and
  exclusions, flag and quality `TransitionMatrix` (SVG grid), per age band
  `Histogram` pairs (published / draft) with median and IQR marks and the baseline's
  acceptable range shaded, `DeltaBars` per metric with CI whiskers, `SweepCurve` for
  sweeps, the provenance block (every RUN-5 field, copyable), actions "Export…" (opens
  the existing export form pre-filled with `analysis_results`), "Re-run", "Cancel"
  (while running), "Promote to draft" (`ExperimentPromote`; opens the version + notes
  dialog; the result links to `/config/scoring/[version]`), and the sessions table
  (cursor-paged, links to session detail). Suppressed cells render as "n < k" with the
  help text, never as zero.
- **RUI-7** Mock mode: `src/lib/api/mock/research.ts` seeds two experiments, two cohorts
  and three runs (one running with progress, one succeeded with suppressed bands, one
  failed `budget_exceeded`) so Playwright covers every state without a backend;
  `e2e/research.spec.ts` and `e2e/live/research.spec.ts`.

## Public Rust API sketch (load-bearing names only)

```rust
// asd-contract/src/params.rs
pub struct ParamSpec { pub id: String, pub document: DocumentKind, pub kind: ParamKind, pub default: Value,
    pub stage: Stage, pub reprocess: Reprocess, pub label_key: String, pub help_key: String,
    pub since: String, pub owner_gate: Option<String> }
pub fn registry() -> &'static [ParamSpec];
pub fn registry_version() -> &'static str;
pub fn validate(document: DocumentKind, body: &Value) -> Vec<ProfileProblem>;   // backend, wasm, accept_bundle

// asd-contract/src/config.rs (1.2.0)
pub struct MetricsParams { pub ivt_velocity_units_per_sec: f64, pub ivt_velocity_deg_per_sec: f64, pub ivt_unit: IvtUnit,
    pub min_fixation_duration_sec: f64, pub fallback_frame_interval_sec: f64, pub smile_reactivity_window_sec: f64,
    pub flat_affect_gain: f64 }
pub struct AoiParams { pub margin_norm: f64, pub dwell_cap_ms: u32 }
pub struct NormsParams { pub min_n: u32, pub include_imputed: bool }
pub struct ScoringProfile { /* 1.1.0 fields */ pub metrics: MetricsParams, pub aoi: AoiParams,
    pub age_bands: Vec<AgeBracket>, pub norms: NormsParams }
pub struct GazeProcessingView<'a> { pub quality_gate: &'a QualityGateParams, pub tracking_quality: &'a TrackingQualityParams,
    pub metrics: &'a MetricsParams, pub aoi: &'a AoiParams }

// asd-contract/src/research.rs
pub struct Experiment { pub id: Uuid, pub name: String, pub status: ExperimentStatus, pub base_profile_version: String,
    pub profile: ScoringProfile, pub profile_hash: String, pub baselines_version: Option<String>,
    pub promoted_config_version: Option<String>, pub last_run_id: Option<Uuid>, pub row_version: i32, .. }
pub struct CohortDefinition { pub age_bands: Vec<String>, pub sex: Vec<Sex>, pub device_ids: Vec<Uuid>, pub from: Option<String>,
    pub to: Option<String>, pub quality_verdicts: Vec<GazeQualityVerdict>, pub tracking_verdicts: Vec<TrackingVerdict>,
    pub min_valid_ratio: Option<f64>, pub scoring_status: Vec<ScoringStatus>, pub require_frames: bool, pub include_synthetic: bool }
pub struct SweepSpec { pub axes: Vec<SweepAxis> }            pub struct SweepAxis { pub param_id: String, pub from: f64, pub to: f64, pub step: f64 }
pub struct AnalysisRunRequest { pub kind: RunKind, pub experiment_id: Uuid, pub cohort_id: Option<Uuid>, pub cohort: Option<CohortDefinition>,
    pub sweep: Option<SweepSpec>, pub level: LevelRequest, pub seed: Option<i64>, pub options: RunOptions }
pub struct AnalysisRun { /* every RUN-1 column except result_blob_key */ pub summary: Option<RunSummary>, pub progress: Option<JobProgress> }
pub struct Dist { pub n: Option<u32>, pub suppressed: bool, pub mean: Option<f64>, pub sd: Option<f64>, pub median: Option<f64>,
    pub iqr: Option<[f64; 2]>, pub p10: Option<f64>, pub p90: Option<f64>, pub ci95_mean: Option<[f64; 2]>, pub hist: Option<Hist> }
pub struct AnalysisExport { pub run_id: Uuid, pub layout: AnalysisLayout, pub encoding: AnalysisEncoding }
// slugs: experiment-name-exists, experiment-promoted, experiment-no-run, cohort-empty, cohort-too-large,
//        sweep-too-large, analysis-quota-exceeded, analysis-level-unavailable, frames-not-available,
//        frames-purged, research-consent-required, digest-required, config-unanalysed, job-finished, result-too-large

// asd-core/src/gaze/reprocess.rs
pub fn reprocess(log: &RawLog, catalog: &StimulusCatalog, view: &GazeProcessingView, options: &ReprocessOptions)
    -> Result<Reprocessed, ReprocessError>;
// asd-core/src/analysis/mod.rs
pub fn evaluate(session: &SessionInput, profile: &ScoringProfile, baselines: &NormativeBaselineSet, level: Level) -> Evaluation;
pub fn suppress(summary: &mut RunSummary, k: u32);
pub struct Aggregator { .. }  // incremental: push(&SessionRow), finish(seed) -> RunSummary

// asd-core/src/metrics.rs
pub fn extract_with(frames: &[GazeFrame], roles: &AoiRole, metrics: &MetricsParams, aoi: &AoiParams) -> Vec<PerVideoMetricSet>;
pub fn from_reports(per_video: &[PerVideoReport]) -> Vec<PerVideoMetricSet>;   // moved from asd-wasm::session

// asd-wasm
paramRegistry() ; reprocessSession(logJsonl, catalog, profile, baselines, options) ; rescoreSession(.. honours profile.metrics/lowConfidenceValidRatio)

// asd-backend
pub async fn run_analysis(ctx: &JobContext, job: &Job) -> Result<Value, JobError>;          // jobs/analysis.rs
impl JobContext { pub async fn progress(&self, done: u64, total: u64, phase: &str) -> AppResult<()>; pub async fn cancelled(&self) -> AppResult<bool>; }
pub async fn seal_research_log(keyring, org, session_id, bytes) -> AppResult<Vec<u8>>;      // routes/blobs.rs (REC-6 helper reused)
```

## Migration 0026 (one file, `0026_research_toolkit.sql`, owner WP-X.0)

`experiments`, `cohorts`, `analysis_runs` (RUN-1, EXP-1, COH-2); `sessions` +
`age_months`, `research_frames_blob_key`, `research_frames_bytes`, `research_frames_sha256`,
`research_frames_purged_at` and the back-fill `UPDATE sessions SET age_months =
(summary->>'ageMonths')::int WHERE summary ? 'ageMonths' AND (summary->>'ageMonths') ~
'^[0-9]+$'`; `organizations.research_frames_retention_months`; `jobs.progress`,
`jobs.cancel_requested_at`; `exports_format_check` + `analysis_results`; indexes
`sessions (org_id, age_months) WHERE research_consented` is not possible (consent is in
JSONB) so `sessions_org_received_idx (org_id, received_at DESC)` plus a partial
`sessions_research_frames_idx (org_id) WHERE research_frames_blob_key IS NOT NULL`.
**B.21 (identity) moves to 0027.**
