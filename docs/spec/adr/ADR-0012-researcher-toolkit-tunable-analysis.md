# ADR-0012: Researcher toolkit: a parameter registry, tunable gaze processing, cohort re-analysis and reproducible runs

Status: Proposed, 2026-11-03 (owner request the same day: "make our admin portal more like a
data scientist tool kit … fine tune or change the range values in a dynamic way for a
researcher to look at the output from the range value and age-driven analysis")
Spec ids: PRM-*, RFL-*, RPX-*, EXP-*, COH-*, RUN-*, SWP-*, AGE-*, PRV-*, GOV-*, RUI-* in
`docs/spec/modules/asd-research-toolkit/technical.md`
Packages: phase 8 (tracker `04 §10`): step 0 WP-X.0; wave 1 WP-X.1, WP-X.2, WP-X.5,
WP-X.6; wave 2 WP-X.3, WP-X.4, WP-X.7, WP-X.8, WP-X.10; wave 3 WP-X.9
Related: ADR-0013 (three-audience platform; records the three principles this ADR
applies: no hard-coded parameters, every string localised, runtime tunability),
ADR-0004 (claims), ADR-0006 (Rust core), ADR-0007 (gaze robustness), ADR-0008/0009
(RBAC, audit), ADR-0011 (clinic server, exports, consent).

## Context

The console already has the seed of a researcher workbench: scoring-profile and
baselines editors validated live by `asd-wasm`, a sandbox that re-scores recent sessions
draft-versus-published (WP-C.5, WP-R.6), a norm-aggregation job with five age bands and
a min-n of 12 (WP-B.5), and de-identified dataset exports (WP-B.9). Everything beyond
that is fixed. What was verified on `dev` at `ee82015`:

- **Fixed constants in the maths.** `asd-core::metrics` hard-codes the I-VT velocity
  threshold (`5.0` normalised units/s), the minimum fixation (`0.08` s), the frame
  interval (`1/60`), the smile-reactivity window (`0.5` s) and the flat-affect gain
  (`50`); `scoring.rs` hard-codes `LOW_CONFIDENCE_VALID_RATIO = 0.60` and the wasm
  rescore deliberately ignores the profile's value; `aoi_space.rs` has no margin;
  `replay()` always uses the default `QualityGate`; the age bands live in
  `norms::default_age_brackets()` and `synth::age_bands()`, not in a document. The
  gaze-robustness thresholds (`TrackingQualityParams`: calibration, validation, drift,
  track state, confidence, gap fill, 1-Euro filter, verdict bands) and the quality gate
  are already profile-delivered (SCH-3, WP-A.13), but the device gets them only through a
  signed bundle and the console shows no UI for most of them.
- **The server cannot read a session's frames.** `frames.jsonl.enc` is sealed with the
  device's Keychain vault key (`SessionVaultManager`), uploaded as an opaque blob, served
  to nobody, and purged after `retention_months` (24). DPIA O-13 asks whether to upload it
  at all. The only server-readable per-session data are the summary, `perVideo` rows and
  `trackingQuality`. The backend never runs the gaze pipeline or `metrics::extract`.
- **Two stored data forms.** Raw tracking logs (`asd-raw-tracking/1`, `RawTrackingSample`:
  face transform, look-at, eye transforms, blinks, light) are written only by the bench
  build, synth and the Android converter. The vault holds processed `GazeFrame`s (x/y,
  valid, AOI names, affect, verdict). Nothing in non-test code builds a `GazeFrame` from
  a `ProcessedSample`; the native side attaches AOI names and the verdict.
- **Governance.** Config documents are global; create, approve and publish require
  `ConfigGlobalManage` (superadmin only) with the four-eyes rule; `ConfigWrite` (owner,
  admin, researcher) exists but no route uses it. Drafts cannot be made by a researcher.
  Only the console enforces "sandbox before publish" (localStorage).
- **Jobs.** One job per worker process, `FOR UPDATE SKIP LOCKED`, heartbeat and reclaim,
  no cancel endpoint, no progress, no per-org quota, no job list. Exports have a
  200,000-row cap and a 500 MB artifact cap. There is no k-anonymity or small-cell rule
  anywhere; the only min-n is the norm cell guard.
- **Age and sex.** Age is only `summary->'ageMonths'`; no column, no sex, no site; the
  dataset export has `age_bracket_at_enrol` but no `age_months`.
- **Console.** React Server Components, server actions, plain CSS and hand-written SVG
  charts (`DriftChart`, `LossBar`, `Distribution`); no chart, table, slider or query
  library; a strict nonce CSP; `researcher` sees exports and every config page.

## Decisions

1. **A self-describing parameter registry in `asd-contract` is the single list of every
   tunable knob (PRM-1..6).** `asd_contract::params::registry()` returns one `ParamSpec`
   per leaf of the configuration documents: `id` (the camelCase dotted path on the wire,
   e.g. `metrics.ivtVelocityUnitsPerSec`), `document` (`scoring_profile` today), `kind`
   (number with `min`, `max`, `step`, `unit`; integer; bool; enum; point list), `default`,
   `stage` (`capture`, `preprocess`, `fixation`, `metrics`, `scoring`, `flag`, `norms`,
   `display`), `reprocess` (what a change needs: `score` = re-score from `perVideo` rows,
   `metrics` = re-extract from frames, `frames` = re-run the gaze pipeline from raw
   samples, `capture_only` = cannot be applied to stored data), `label_key` and
   `help_key` (message keys, never literals, ADR-0013 principle 2), `since` (profile
   schema version) and `owner_gate` (a knob the owner has fenced, e.g. SCR-* behind
   H-G2, is listed but marked). Validation (`params::validate(document, json)`) checks
   every value against its spec and then runs the existing cross-field checks
   (`profile_checks`, `TrackingQualityParams::validate`), so backend publish, wasm
   editors and `asd_sync::accept_bundle` refuse the same things. A test asserts every
   serialised leaf of the default documents is registered exactly once and every
   registered id resolves to a leaf, so a field added to a struct without a spec fails
   the build. `asd-cli schema export` writes `docs/spec/schemas/asd/param-registry-v1.json`;
   the console, iOS and Android render tuning UIs from it generically (one component per
   `kind`, none per knob).
2. **One document, `scoring_profile` 1.2.0, carries every processing and scoring knob;
   there is no separate `GazeProcessingProfile` document.** The profile gains `metrics`
   (I-VT velocity and unit, minimum fixation, fallback frame interval, smile-reactivity
   window, flat-affect gain), `aoi` (hit-test margin in normalised units, dwell cap),
   `ageBands` (the five default brackets as data) and `norms` (`minN`, `includeImputed`),
   all `serde(default)` to today's constants so a 1.1.0 profile decodes unchanged and
   every golden vector stays byte-identical. `qualityGate`, `trackingQuality`, `metrics`
   and `aoi` together are what the code calls the gaze-processing profile
   (`asd_contract::config::GazeProcessingView`, a borrowed view), and the registry marks
   them `reprocess = frames|metrics`. One document because the device builds its whole
   pipeline configuration from one profile (`gaze_config_json_for_profile`), the four-eyes
   lifecycle is per document, and two documents would need a pairing rule for "which gaze
   profile with which scoring profile" in every session, run and export. `asd-core` reads
   these values from the profile (`metrics::extract_with(frames, &MetricsParams, &AoiParams)`,
   `scoring::score_session_with(.., low_confidence_valid_ratio)`, `replay()` with a gate)
   and its constants become the `Default` impls only.
3. **A consent-gated research frame log makes frame-level reprocessing possible
   (RFL-1..8).** The device writes, per session, a server-readable log in the existing
   raw format (`asd-raw-tracking/1`: header with device geometry, orientation and scheduled
   targets; one `RawTrackingSample` per frame) extended additively with `header.clips`
   (clip id, order, catalog version, AOI set used), `header.bundleVersion`,
   `header.profileVersion`, and per-sample `affect` (the three affect numbers). It is
   uploaded only when the norm-collection gate is active (research consent + opt-in +
   real capture, the same gate as `normCollection.consented`), through the proxy blob
   route only (never presigned), with `Content-Digest` required and a 16 MB cap; the
   backend seals it under the org DEK before the blob store (AAD
   `sessions:research_frames:org:id`, the attachments pattern) and records
   `sessions.research_frames_blob_key`. It is subject to its own retention
   (`organizations.research_frames_retention_months`, default = `retention_months`), is
   deleted by erasure, and every read is audited. It is **not** the vault file: raw
   samples, not processed frames, so every stage after capture can be re-run, and the
   on-device pipeline over the same samples is byte-identical to the server replay (WP-G.7
   parity). O-13 gets a recommendation: once the research log ships, stop uploading
   `frames.jsonl.enc` (open question 4).
4. **Three reprocessing levels, chosen per session, never silently mixed (RPX-1..6).**
   `score`: from the stored `perVideo` rows (every scored session; today's sandbox);
   `metrics`: not offered on its own (processed frames are not server-readable;
   subsumed by `frames`); `frames`: `asd_core::gaze::reprocess(log, catalog, profile,
   baselines, age)` runs replay → AOI mapping from the catalog → `TelemetryWriter` →
   `GazeFrame`s → metrics → score → flag → tracking summary, and returns the metrics,
   score, flag, quality verdicts and an optional down-sampled trace for plotting. A run
   declares `level = auto|score|frames`; `auto` uses `frames` where a log exists and
   `score` elsewhere, and every result row says which level produced it. Sessions whose
   log was never uploaded, was swept, or whose knobs changed are `capture_only` are
   reported as excluded with a reason (`no_frame_log`, `frames_swept`, `capture_only`),
   never approximated. The same function is exported through `asd-wasm`
   (`reprocessSession`) for the single-session live preview and through `asd-cli research
   reprocess` for the terminal.
5. **Experiments, cohorts and analysis runs are org-scoped records outside the config
   lifecycle (EXP-*, COH-*, RUN-*).** An **experiment** is a named draft profile (full
   `scoring_profile` body, base version recorded, optional draft baselines version) that a
   researcher edits with the registry UI and previews in the browser. A **cohort** is a
   saved filter (age bands, sex when the session carries it, sites = devices, date range,
   quality verdicts and tracking verdicts, minimum valid ratio, scoring status, capture
   source real only; research consent is always required and not a choice) with a JCS
   hash. An **analysis run** is a backend job of kind `cohort_rescore` (phase A),
   `param_sweep` or `age_profile` (phase B) that applies an experiment to a cohort against
   the published profile and writes: aggregates per age band (distributions, percentiles,
   per-metric deltas, flag and quality transition matrices, counts by level) into
   `analysis_runs.summary`, and per-session rows into a DEK-sealed result blob. Jobs
   stream sessions in pages, aggregate incrementally, heartbeat, report progress, honour
   cancel, and stop at a budget. Nothing here changes a stored session, a flag, a review
   or a baseline.
6. **Parameter sweeps are bounded sensitivity analyses (SWP-1..4).** One or two registry
   knobs, each with `from`, `to`, `step` (≤ 25 steps each, ≤ 625 cells); the result is,
   per cell, the flag rate, the quality-status rate and the mean and median of chosen
   metrics, with the published value marked. Budgets: ≤ 200,000 session evaluations at
   `score` level, ≤ 20,000 at `frames` level, 30 minutes wall clock; a request over
   budget is refused before it is queued (`sweep-too-large`), never truncated.
7. **Age-driven analysis uses the profile's age bands, the published or a draft baseline,
   and a minimum-n rule with small-cell suppression (AGE-*, PRV-*).** Distributions and
   percentiles per band per metric are reported against the baseline's acceptable range;
   "how a draft baseline would move them" is a `cohort_rescore` with `baselines_version`
   set to a draft. Privacy rule for every aggregate the toolkit shows or exports: a cell
   (band × sex × site × metric) with fewer than `k` sessions is suppressed (`n: null,
   suppressed: true`), p10/p90 need `n ≥ 20`, median and IQR need `n ≥ k`; `k = 12` by
   default (the norm min-n), org-settable in `[5, 50]`, never below 5 (open question 2).
   Per-session rows are shown in the console only to roles that may read sessions
   anyway, and leave the server only through the export job: de-identified by `deid.rs`
   with a fresh salt, `age_months` reduced to the band, dates to `YYYY-MM`, no device id.
8. **Every run is reproducible and re-openable (RUN-5..8).** A run stores the experiment
   profile snapshot and its JCS hash, the published profile, baselines and catalog
   versions, the cohort definition and its hash, the membership hash (sorted session ids),
   the levels used, `asd-core` and contract versions, `PIPELINE_VERSION`, the registry
   version, and a 64-bit seed (used for bootstrap confidence intervals and subsampling;
   random unless supplied). `rerun` creates a new run from the stored definition and
   reports membership drift. Results export through the existing export job as
   `ExportFormat::analysis_results` (JSONL or CSV; Parquet for per-session rows reuses the
   dataset encoder) with the run's provenance block in the manifest.
9. **Governance: tuning never reaches clinical output except through
   publish-with-approval (GOV-1..6).** Experiments affect only analysis runs and
   previews. "Promote to draft" creates a `scoring_profile` (or `normative_baselines`)
   draft document through the existing lifecycle with the experiment id and the last
   run id in its notes; it needs `ConfigWrite` (owner, admin, researcher), which the draft
   route starts honouring (approve, publish and retire stay `ConfigGlobalManage` with
   four-eyes). The console's "sandbox before publish" rule becomes a server rule: a
   candidate may be published only when a finished analysis run references its body
   hash (open question 3 decides the strength: refuse or warn). New RBAC actions:
   `AnalysisRead` (owner, admin, clinician, researcher, superadmin), `AnalysisRun`,
   `ExperimentWrite`, `ExperimentPromote` and `FramesRead` (owner, researcher). The vendor
   (superadmin) never runs analyses or reads frame logs. Audits: `experiment.*`,
   `cohort.*`, `analysis.run_enqueue|run_complete|run_cancel|read`, `session.frames_read`,
   `session.frames_upload`. Resource limits fit one clinic box: one running analysis per
   org, five queued, one analysis at a time per worker, the budgets of decision 6, a 64 MB
   result blob, and progress visible in the console. Reads of analysis results are
   audited once per run detail because one page covers many participants.
10. **Console UX for a data scientist, without new runtime libraries (RUI-1..7).**
    `/[org]/research` with experiments, cohorts and runs. The workbench renders registry
    groups as sliders with numeric inputs and unit labels, greys out `capture_only` knobs
    in preview, shows a session picker (recent, research-consented, with a "frames"
    badge where a log exists) and a live preview panel that re-runs `rescoreSession`
    (150 ms debounce, the `useCoreCheck` pattern) or `reprocessSession` in WASM and
    plots the scanpath, fixations, AOI dwell bars, per-clip subscores and the flag,
    published versus draft side by side. "Run on cohort" opens the cohort picker and
    queues a run; the run page shows progress, then histograms per age band, delta bars,
    transition matrices and sweep curves as inline SVG (the existing chart pattern; the
    CSP and licence rules stay), with "Export", "Re-run", "Promote to draft". Every string
    goes through `src/messages/*` with `en` and `ne` keys from the start.
11. **Phasing.** Phase A (waves 1 and 2) delivers: registry + profile 1.2.0 + constants
    moved; research frame log on iOS and the backend; `reprocess` in core, wasm and CLI;
    the workbench with live preview; cohorts; `cohort_rescore` with age-band distributions,
    deltas and transition counts; suppression; audits and RBAC; JSONL/CSV export; re-open.
    Phase B (wave 3): sweeps, `age_profile` runs, Parquet, Android frame log once an
    Android app exists.

## Alternatives rejected

- **A separate `gaze_processing_profile` document.** Cleaner naming, but it would split
  what the device already consumes as one object, add a pairing problem in every session
  record and export, and double the lifecycle work for a researcher who tunes both sides
  in one experiment. Rejected; the registry's `stage` field gives the separation for
  free.
- **Reprocess from the vault file by escrowing the device key.** Key escrow puts a
  biometric decryption key on the clinic server for every session, including those
  without research consent, and needs a new key-management ADR (ADR-B3). The research
  log is uploaded only with research consent, in clear over TLS, sealed by the server:
  the same posture as attachments, and nothing changes for families who did not consent.
- **Upload processed `GazeFrame`s instead of raw samples.** Smaller, but filter, gate,
  calibration model, gap fill and AOI mapping could not be re-tuned, which is most of
  what the owner asked for. Raw samples are ~3 MB per session and the pipeline replays
  them at ~0.0013 ms per frame.
- **Run cohort jobs in the browser (WASM).** Fine for one session; a 500-session frame
  cohort is 1.5 GB of decrypted logs through a browser tab, no audit of what was read, and
  no reproducible record. Jobs stay on the server; WASM is the live preview.
- **A notebook or Python side-car for researchers.** `CLAUDE.md` forbids new Python;
  it would also be a second implementation of the maths and a hole in the audit and
  de-identification rules. Researchers who want their own tools get the de-identified
  JSONL/Parquet export and, later, the registry and `reprocess` through `asd-cli`.
- **Store per-session results in Postgres rows.** 2,000 sessions × 625 sweep cells is a
  million rows per run on a one-box clinic server. Aggregates in JSONB, per-session rows
  in one sealed blob with a cursor endpoint.
- **Recharts/visx/shadcn for the charts.** Would be the first client chart library,
  must pass the nonce CSP and the MIT/Apache rule, and the existing SVG components already
  cover lines, bars and distributions. Revisit when a chart needs interaction the SVG
  pattern cannot give (brushing, zoom).
- **Letting experiments publish directly from the workbench.** The owner's rule is that
  tuning never changes clinical output without approval; the four-eyes lifecycle stays
  the only path, and the toolkit only files a draft with its evidence attached.
- **Differential privacy on aggregates.** Right tool for public release, wrong for a
  clinic's own research console with ≤ a few thousand sessions: it would blur exactly the
  small differences the researcher is looking for. k-anonymity-style suppression plus
  research consent plus audited access is the rule here; DP is a later ADR if aggregates
  are ever published outside the org.

## Consequences

- `scoring_profile` schema 1.2.0; the config bundle and `api-v1.json` gain the new
  sections and the registry schema file; `asd-cli schema check` covers
  `param-registry-v1.json`. Device bundles signed before the change still verify and
  decode (defaults), so no app release is required for phase A; the iOS research log
  writer (WP-X.6) is an app change that only consented research sessions exercise.
- `asd-core` constants become defaults; a handful of functions gain `_with` variants
  that take params; the golden vectors prove the defaults are unchanged.
- The backend gains migration **0026** (experiments, cohorts, analysis runs, research
  frame columns, `jobs.progress`/`cancel_requested_at`, `sessions.age_months`, the export
  format) — B.21 (identity) moves to **0027** because it is blocked on an owner decision
  and a gap in the applied sequence must not exist (tracker rule).
- `asd-wasm` grows by `paramRegistry` and `reprocessSession`; `rescoreSession` starts
  honouring `lowConfidenceValidRatio` and the `metrics` section (an R.6 open item closes).
- The worker gains cancel and progress for every job kind; the console gains one
  reusable job-polling hook.
- `researcher` becomes a working role: experiments, cohorts, runs, frame reads, draft
  proposals, exports. It still sees no clinical content (RBAC-2 unchanged).
- The research log adds ~3 MB per consented session to the blob store; with its own
  retention default equal to `retention_months` the growth is bounded by the existing
  policy; O-3 should state a number.
- `ageMonths` becomes a column (`sessions.age_months`, back-filled), which the dataset
  export may later expose as a band; `age_bracket_at_enrol` stays.
- The claims gate (ADR-0004) applies to the research pages: metrics, flags and
  "distribution" are research vocabulary; "risk", "diagnosis", "screening verdict" are
  not used, and every results page carries the mandatory sentence.

## Open questions for the owner (recommended default in brackets)

1. **Research frame log retention** (O-3 adjacent): same as `retention_months` (24) or
   shorter [same; separate setting exists for the day it should differ].
2. **Small-cell threshold `k`** [12, matching the norm min-n; org setting bounded to
   5..50].
3. **"Analysis before publish" strength**: refuse to publish a candidate profile without
   a finished run over its body hash, or warn only [refuse; the run can be tiny].
4. **O-13: keep uploading the device-sealed `frames.jsonl.enc`** once the research log
   exists [stop in the app release that ships WP-X.6; retention sweep keeps purging old
   ones].
5. **Who may promote an experiment to a draft** [owner, admin, researcher via
   `ConfigWrite`; approve and publish unchanged: superadmin, four-eyes].
6. **Clinic server without a vendor account**: on a single-clinic box nobody holds
   `ConfigGlobalManage`. Allow the org `owner` to approve and publish under four-eyes with
   a second owner/admin, or keep publishing a vendor act [keep vendor for the pilot; add
   an `org.local_publish` setting in a later package if a clinic runs without the vendor].
7. **Sweep and cohort budgets** [as decision 6; `ASD_ANALYSIS_*` env knobs so a bigger box
   can raise them].
8. **Sex in the session record**: add a `sex` field to session schema 1.9 so cohorts can
   filter on it [yes, optional, `unknown` default, in the same release as WP-X.6; until
   then the sex filter is absent, not fabricated].
9. **Owner gate H-G2 (SCR-*)**: the registry lists `trackingQuality.scoring.*` as
   `owner_gate`; the toolkit lets a researcher sweep them in analysis only [yes; that is
   exactly the evidence the gate asks for].
