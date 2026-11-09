# ADR-0015: Research workbench v2: scientific depth, dataset curation, standard research formats

Status: proposed, 2026-11-09. Owner ask: "a robust app with graph data at scientific level of detail,
fine-tune options for datasets, curves and helpers, and import/export to standard research formats".
Builds on ADR-0012 (researcher toolkit) and ADR-0014 (open source and standards). Decisions marked
**owner** need an answer before their package starts.

## Context: what the toolkit already does (phase 8, landed)

| Capability | Where |
| --- | --- |
| Self-describing parameter registry; every processing and scoring knob in `scoring_profile` 1.3.0 | `asd-contract`, `asd-core::params` |
| Consent-gated frame log; three reprocessing levels (frames, metrics, scores) never mixed silently | `asd-core::gaze::reprocess`, iOS/Android loggers |
| Experiments, cohorts, analysis runs (cohort re-score, 1–2 axis parameter sweeps ≤ 625 cells, age profile) with progress, cancel, budget, seeded bootstrap 95 % CIs, small-cell suppression (n ≥ 12) | `asd-core::analysis`, backend `analysis_run` job, console `/research` |
| Promote a run to a draft scoring profile or draft baselines; publish only through config governance | WP-X.2, WP-X.9 |
| Exports: aggregates (CSV, JSONL, Parquet), per-session rows (JSONL, CSV, Parquet with footer provenance), de-identified, manifest with run provenance | `asd-export::analysis`, `dataset.rs` |
| Live preview in the browser through the Rust core compiled to WebAssembly (same code as the server) | `asd-wasm` |
| Charts: inline SVG (histograms with baseline range, delta bars with CI, transition matrices, sweep curves and heatmaps, scanpath) | `components/research` |

What a data scientist still cannot do: freeze and version a dataset with exclusion rules, labels and splits;
compare groups with effect sizes and corrected p-values; fit age percentile curves with confidence bands;
check agreement between device models or app versions; evaluate a classifier against clinician labels with
ROC/PR and calibration; import gaze data from other instruments or standard layouts; export in a format a
reviewer or another lab opens without reading our docs; download a chart as SVG/PNG or as a reusable spec.

## Decisions

### A. Where the science lives

| # | Decision | Default / owner |
| --- | --- | --- |
| R-1 | Every statistic, fit and chart dataset is computed in **Rust** (`asd-core::analysis`, new `asd-core::stats`, `asd-core::viz`) and reaches the console through WASM (preview) and the backend run job (full cohorts). No numeric code in TypeScript beyond rendering. (Repo rule.) | default |
| R-2 | **Statistics pack** (`asd-core::stats`): ECDF, kernel density (Gaussian, Silverman bandwidth), QQ against normal, Welch t, Mann–Whitney U, Hedges g, Cliff's delta, Holm–Bonferroni, Bland–Altman limits of agreement, ICC(2,1) for test–retest, bootstrap and BCa intervals, seeded. Pure Rust, no new crates beyond `rand` (already present); property tests against reference values. | default |
| R-3 | **Age percentile curves**: LMS method (Box–Cox power, median, coefficient of variation smoothed by penalised cubic splines over age in months), 3rd/10th/25th/50th/75th/90th/97th centiles with bootstrap bands; falls back to binned quantiles (today's age profile) under n thresholds. Output can be promoted to draft baselines exactly like an age-profile run. | default; **owner**: minimum n per age month for a fit (recommend 60 per band, suppression stays n ≥ 12) |
| R-4 | **Agreement and comparison run kinds**: `group_compare` (two cohorts or two profile hashes, per metric: distributions, effect size, corrected p), `agreement` (same participant, two devices/app versions: Bland–Altman, ICC), both stored as runs with provenance like today. | default |
| R-5 | **Classifier evaluation** only on a dataset that carries clinician labels from the review queue: ROC and PR curves with bootstrap bands, calibration (reliability diagram, Brier), threshold table. Results can never flow into a scoring profile except through promote → draft → publish (ADR-0012 governance). The device never receives a classifier the console has not published. | default |
| R-6 | **Gaze-level helpers**: I-DT as an alternative to I-VT for fixation detection (registry params), time-to-first-fixation and dwell-time curves per AOI over clip time, cohort heatmaps per clip (binned, suppressed), scanpath similarity (MultiMatch) deferred. | default |

### B. Datasets as first-class, immutable objects

| # | Decision | Default / owner |
| --- | --- | --- |
| R-7 | A **dataset** = frozen member list of sessions (from a cohort at a point in time) + exclusion rules (quality thresholds and manual exclusions with reasons) + optional label set + optional split (train/validation/holdout, stratified by age band and seeded) + data dictionary. Immutable once frozen; identified by a content hash; lineage records the cohort, profile hash, run and parent dataset. New versions, never edits. | default |
| R-8 | Labels come from clinician review (existing review queue outcomes) or from an imported label file; each label row carries source, time and reviewer pseudonym. Datasets with labels are marked and their exports carry the label provenance. | default |
| R-9 | Dataset membership snapshots respect erasure: an erased session leaves `excluded.erased` in the manifest, never its data (ADR-0012 RUN-8 carried over). | default |

### C. Import and export in standard formats

| # | Decision | Default / owner |
| --- | --- | --- |
| R-10 | **Export package** = Frictionless **Data Package** (`datapackage.json` with Table Schema per resource) containing CSV or Parquet resources, the data dictionary, `provenance.json` (dataset hash, profile hash, code version, seed, run id), a `README.md` and `CITATION.cff`; optional **Croissant** JSON-LD for ML datasets and **DataCite** metadata JSON for a DOI deposit (Zenodo). Loader snippets for R (`frictionless`) and Python (`frictionless`, `pandas`) in the README; no Python code in the repo. | default |
| R-11 | **Frame-level export** follows the **BIDS eye-tracking** layout (`*_eyetrack.tsv.gz` + `*_eyetrack.json` sidecar with sampling rate, screen geometry, AOI definitions in `*_events.tsv`), one subject pseudonym per participant, under `derivatives/`. | default; **owner**: confirm BIDS eye-tracking as the frame-level standard (alternative: our JSONL only) |
| R-12 | **Import**: (a) our own raw frame logs (JSONL, iOS and Android, exists); (b) BIDS eye-tracking datasets; (c) generic CSV gaze samples through a column-mapping wizard (time, x, y, validity, optional pupil), with sampling-rate detection and unit choice; (d) participant metadata CSV (pseudonym, age in months, optional sex) and label CSV. Every import is de-identified at the door (names and dates of birth rejected by schema), org-scoped, audited, and recorded as a dataset with `source.kind = import`. Imported sessions never enter the clinical review queue. | default; **owner**: is import of non-EarlyGaze instruments in scope for 2027 Q1 (recommend yes for BIDS and CSV, no for proprietary EDF) |
| R-13 | Clinical interoperability stays FHIR/HL7 (ADR-0014); research formats are the ones above. OMOP CDM mapping is deferred until a partner asks. | default |

### D. Charts

| # | Decision | Default / owner |
| --- | --- | --- |
| R-14 | Charts are described as **Vega-Lite** specs generated in Rust (`asd-core::viz` emits data + spec; the console only renders). Console renders with `vega-embed` (BSD-3), themed by the design tokens, with Download as SVG, PNG and spec (opens in Altair, Observable, Vega Editor). Existing SVG charts migrate incrementally; axes, units, n and CI method are always printed on the chart; colour never carries meaning alone. | **owner**: Vega-Lite (recommended) vs Plotly (MIT, heavier) vs keep hand-made SVG |
| R-15 | Every chart has a table view and a CSV download of exactly the plotted data (reproducibility; WCAG). | default |

### E. Domain visualisations: gaze replay, expected-versus-actual, outlier tools (owner, 2026-11-09)

Owner: "more custom graphics for our domain: replay of where the child looked on the video, the baseline
limit, the expected level on the axis, outlier filters in the data tools". These are the views an
eye-tracking researcher and a clinician actually use; the statistics pack serves them, not the other way.

Data already stored per session: per-frame `x_norm`, `y_norm`, `is_valid`, `video_time_sec`,
`clip_media_time_sec`, `active_aoi_names`, affect, quality verdict (`GazeFrame`, in the session's
`rawPayload`, readable by the console through `GET /sessions/{id}/raw` with an audit row); the consent-gated
research frame log (raw samples, head and eye transforms, blinks); the catalog's time-coded AOI boxes with
roles (`Area`); the device geometry (screen mm, pixels); the published baselines and flag thresholds per age
bracket. Not stored on the server: the clip media (licensed, H-8) and the device key that seals the
`frames` blob.

| # | Decision | Default / owner |
| --- | --- | --- |
| V-1 | **Session replay** (session detail "Replay" tab; the research workbench preview): a canvas showing the clip media when the org has uploaded it (V-8), otherwise a wireframe of the clip's AOI boxes; the gaze point with a 300 ms trail; fixations as circles whose radius is duration, detected by the **current profile's parameters through WASM** (so changing the I-VT threshold re-renders them); saccades as lines; blink and lost segments greyed on the trail; AOI boxes coloured and labelled by role; a scrubber bound to `clip_media_time_sec`; play at 0.25x to 2x; frame step. Under the canvas, a timeline strip: an **AOI-hit ribbon** (which AOI the gaze is in at every instant, by role colour), the existing tracking-loss bar, scheduled targets and clip boundaries. | default |
| V-2 | **Expected versus actual on the time axis**: per clip, the metric trace over clip time (for example proportion of gaze on the target AOI in a sliding 1 s window) with the **expected band** shaded from the published baselines for the child's age bracket (mean ± tolerance, or the percentile band once R-3 lands) and the flag thresholds drawn as labelled horizontal lines ("expected 0.62 for 24–35 months", "flag below 0.35"). The clip card shows recorded versus expected as a bar inside the band. The axis never implies a diagnosis; labels say "expected range", "recorded". | default |
| V-3 | **Heatmaps**: per clip, the session's gaze density (Gaussian kernel, grid computed in Rust) over the clip frame or AOI wireframe, side by side with the age-bracket cohort density from a run or baseline set, and a difference map; n and suppression printed on the cohort panel. | default |
| V-4 | **Quality timeline** (extends WP-G.12): distance, head yaw and pitch, calibration drift, luminance, each with its gate threshold drawn from the effective profile, aligned to the replay scrubber so a quality dip can be seen against the gaze. | default |
| V-5 | **Outlier and filter tools** (dataset and run level, R-7): per metric a histogram with brushable thresholds; rule presets: quality gates from the profile, Tukey 1.5×IQR, MAD-based robust z > 3.5, minimum valid-sample ratio, minimum clips completed; live n before and after; each exclusion stored with its rule or reason in the dataset; a **what-if** panel shows the aggregates with and without the excluded sessions (sandbox, no writes). | default |
| V-6 | **Tuning view**: the workbench's single-session preview gains the replay and the timeline, side by side for published and draft parameters, so a researcher watches fixations and AOI hits change while moving a knob. The sweep heatmaps (exist) link each cell to this view. | default |
| V-7 | **Clinician read-out**: one consistent "centile strip" per metric: the age-bracket distribution with the published band and a dot for this session, plain words, no colour-only meaning; used on the session detail and in the review queue instead of ad-hoc badges. | default |
| V-8 | **Clip media on the server**: an admin uploads each licensed clip (mp4) through Catalog, per clip id; the file goes to the org blob store sealed under the org DEK with its sha256 and licence acknowledgement recorded; served to roles with `session.read` through a short-lived, audited URL; the replay degrades to the wireframe when a clip is missing. The media never leaves the clinic server. | **owner**: confirm server-side hosting is allowed under the clip licence (H-8) |
| V-9 | **All computation in Rust** (`asd-core::replay`: fixation and saccade events from frames with profile parameters, AOI-hit ribbon, windowed metric traces, KDE grids, outlier rules), through WASM for one session and the backend job for cohorts; the console draws canvas and SVG only. Replay reads use the existing `session.raw_read` audit; cohort heatmaps obey suppression. | default |

## Work packages

| WP | Scope | Tier | Depends on |
| --- | --- | --- | --- |
| WP-X.11 | Dataset entity: contract, migration, backend CRUD + freeze + lineage, console pages (`research/datasets`), labels from review outcomes, splits | S | — |
| WP-X.12 | `asd-core::stats` + WASM bindings + run kinds `group_compare`, `agreement`; console result pages with tables | S | — |
| WP-X.13 | LMS percentile curves (`age_profile` v2) with bootstrap bands; promote to draft baselines | S | X.12 |
| WP-X.14 | `asd-core::viz` Vega-Lite spec emission; `vega-embed` in the console (CSP check, bundle size); downloads; migrate existing charts | S | owner R-14 |
| WP-X.15 | Export package: Frictionless Data Package, Croissant, DataCite, README + CITATION, BIDS eye-tracking derivatives | I | X.11; owner R-11 |
| WP-X.16 | Import wizard: CSV mapping, BIDS import, participants/labels CSV; de-id schema at the door; audit | S | X.11; owner R-12 |
| WP-X.17 | Classifier evaluation on labelled datasets (ROC/PR/calibration with bands) under governance | S | X.11, X.12 |
| WP-X.18 | Gaze helpers: I-DT, time-to-first-fixation and dwell curves (feeds V-2) | S | X.19 |
| WP-X.19 | `asd-core::replay` + WASM: fixation/saccade events from frames with profile params, AOI-hit ribbon, windowed metric traces with expected bands, KDE heatmap grids, outlier rules; reference tests | S | — |
| WP-X.20 | Console replay: Replay tab (canvas, scrubber, trail, fixations, AOI boxes, timeline strip), expected-versus-actual charts on clip cards, heatmap pair, quality timeline alignment, tuning view side by side | S | X.19 |
| WP-X.21 | Clip media: catalog upload (sealed, sha256, licence ack), audited short-lived serving, replay media source | S | owner V-8 |
| WP-X.22 | Outlier and filter tools on datasets with what-if aggregates | S | X.11, X.19 |
| WP-X.23 | Centile strip component replacing ad-hoc badges on session detail and review | I | X.19 |

Order: X.19 and X.11 start now in parallel (no owner input needed), then X.20 and X.22; X.12, X.14 after R-14;
X.13 after X.12; X.15 after X.11; X.21 after V-8; X.16, X.18, X.23, X.17 next. Each package: spec first, Rust tests with reference values, WASM
parity test (native == wasm32), console e2e, exports validated with the `frictionless` CLI in CI
(dev tooling only).

## Consequences

- The console becomes a reproducible analysis environment: every chart and table traces back to a dataset
  hash, a profile hash and a seed, and leaves the building in formats other tools open.
- One new runtime dependency in the console (`vega-embed`) if R-14 is accepted; everything numeric stays
  in the Rust core as the repo rule requires.
- Datasets add a table and storage for membership snapshots; erasure handling is unchanged.
- None of this touches the App Store build.
