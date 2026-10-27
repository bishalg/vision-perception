# 03 — ASD Clinic / Research Admin Console: Feature Spec

> Working name: **Curv Console**. A web app for clinic admins, clinicians, reviewers and researchers. Backend contract in `02`. Work packages WP-C.* in `04`.
> Not to be confused with `docs/admin_dashboard.md`, which is the static palm/flood F1 dashboard; only its *human-validation-queue* idea is reused here.

## 1. Users and jobs-to-be-done

| Role | Needs |
|---|---|
| **Org owner / admin** | Enrol devices, invite users, set retention, see usage, manage config versions, export data, handle erasure requests. |
| **Clinician** | Find a participant, see their sessions over time, open a session (per-clip matrix, flag, quality, replay heatmap), download PDF/FHIR, add notes. |
| **Reviewer** (QA) | Work the review queue: confirm quality verdicts, label sessions/clips (good capture / child distracted / adult in frame / calibration drift), dismiss false reds. |
| **Researcher / data scientist** | Compose cohorts, run norm aggregation, compare candidate baselines vs published, tune scoring profile in a sandbox, export datasets with labels, manage model registry entries. |
| **Superadmin (you)** | Everything, across orgs; feature flags; system health. |

## 2. Decisions

- **Stack:** Next.js 16 (App Router) + TypeScript + shadcn/ui + TanStack Query + Recharts; auth via backend JWT (httpOnly cookie). Deployed on Vercel (preview per PR) or as a static export served by the API container for air-gapped clinics. Reason: fastest path to a polished, mobile-friendly console; the repo already has a React/Vite TS app (`apps/annotation_studio`) so patterns transfer, and `vercel:*` skills exist in the owner's tooling.
  - Alternative considered: extend `apps/annotation_studio` (Vite). Rejected: it is palm-centric and lacks routing/auth; keep it separate.
- **Location:** `apps/curv_console/` (pnpm workspace member; root `package.json` gets `console:dev`, `console:build`).
- **Design:** reuse the app's "Liquid Glass" tone loosely but prioritise density and clarity (data tables). Dark/light. English first; strings in a `messages/` folder for later hi/ne.
- **Never shows:** names, DOB, photos, raw frames. Participant identity is the pseudonymous token plus an optional clinic-entered *local reference* field stored only in the org's row (encrypted column, labelled "never a name").

## 3. Information architecture

```
/login
/[org]/overview                 KPIs: sessions (7d/30d), flag distribution, quality distribution, review queue size, devices active, config adoption
/[org]/participants             table: token, local ref, age bracket, sessions n, last flag, last session, consent version
/[org]/participants/[id]        timeline of sessions (composite over time, flags), care events summary, erasure button (admin)
/[org]/sessions                 filters: date, flag, quality, battery, age pack, bundle version, device, reviewed?
/[org]/sessions/[id]            header (flag, composite, confidence, quality, bundle_version) · tabs: Clips (expected-vs-recorded table + per-clip deviation chart) · Quality (valid ratio, distance, pose, calibration) · Affect · Replay (heatmap/flow per clip from frames blob — Phase 2, requires decrypt path or client-side upload of a down-sampled replay) · Report (PDF viewer, FHIR JSON) · Review (labels, notes, history) · Audit
/[org]/review                   queue with reasons, assignment, SLA age; bulk actions; keyboard-driven triage
/[org]/config/catalog           versions list → editor: clips (id, title, age packs, languages, duration, AOI roles + time-coded AOI rects with a timeline editor over the clip preview), batteries (express/comprehensive ordering)
/[org]/config/baselines         versions list → matrix editor: 10 clips × 5 brackets × metrics, cell provenance (literature / interpolated / own-norm run n=…), direction, tolerance; diff vs published
/[org]/config/scoring           versions list → form: status bands, piecewise score breakpoints, weights per clip/metric, composite confidence gate (0.60), flag thresholds (0.65 / 0.35 / 40 %), quality gate params (distance, yaw, pitch, confidence, blink)
/[org]/config/bundles           compose catalog + baselines + scoring → preview → sign + publish; adoption chart (which app versions pulled which bundle)
/[org]/config/sandbox           "Re-score" tool: pick N sessions, pick a draft bundle, see how flags/composites would change vs published (no writes) — the key safety tool before publishing a scoring change
/[org]/research/cohorts         saved filters; counts per cell (age × clip × metric) with n≥12 indicator
/[org]/research/norm-runs       run aggregation on a cohort → cells table → promote to draft baselines (with provenance written into each cell)
/[org]/research/exports         dataset export jobs (parquet/jsonl, with labels), download links, hash
/[org]/research/models          model registry: age-band model, attention classifier (future); fields: version, training data ref (export job id), licence, metrics (recall per band), file checksum, status (experimental / shipped-in-app-version)
/[org]/devices                  enrolled devices, app versions, last sync, revoke, generate enrol code (QR)
/[org]/users                    invite, roles, TOTP status, disable
/[org]/settings                 retention months, consent version text, local-ref policy, export policy
/[org]/audit                    searchable append-only log
/admin                          superadmin: orgs, feature flags, system health (queue depth, job failures, DB size), config signing key rotation
```

## 4. Key screens in detail

### 4.1 Session detail — Clips tab
Table rows = clips in play order. Columns: clip, segment (time-coded content from catalog), metric, expected (baseline for bracket, with provenance chip), recorded, deviation, status (onTarget/monitor/atypical), weight, contribution to composite. Row click expands the deviation chart. Footer: composite, confidence, flag, bundle version, "Re-score with draft bundle" (opens sandbox prefilled).

### 4.2 Review queue
Reasons auto-generated by backend rules (configurable in scoring profile): `quality_low`, `flag_red`, `random_audit (5 %)`, `norm_candidate (consented + opted-in + non-sim)`, `device_first_session`. Triage actions: *Confirm*, *Flag capture issue* (pick label), *Exclude from norms*, *Dismiss*. Every action writes `labels` + `audit_log`. Keyboard: j/k, 1–4.

### 4.3 Catalog editor (stimulus configuration)
- Clip list with upload (mp4/mov) → server measures real duration + checksum (fixes the "every clip is 8.0 s" problem at the source) → licence/provenance fields mandatory before a clip can be in a *published* catalog.
- AOI editor: timeline + canvas over the clip; define rect AOIs with role (social/geometric/eye/mouth/orienting) and time ranges; export matches `stimulus_catalog.json` schema used by `CurvAsdGazeKit`.
- Battery composer: drag clips into Express (4) / Comprehensive (8) per age pack × language; validation errors (missing language, duration ≠ catalog) shown inline.

### 4.4 Baselines editor
Grid per clip: rows = age brackets, columns = metrics. Each cell: expected value, tolerance/direction, provenance (`literature:<citation>` | `interpolated` | `own:<norm_run_id>, n=`), confidence. Colour cells by provenance so imputed cells are obvious. "Diff vs published" view. Import from JSON (current `normative_baselines.json`) to bootstrap v1.

### 4.5 Scoring profile editor + sandbox
Form-driven; every change previews on a sample of recent sessions (sandbox) before "Save as draft". Publishing requires: at least one sandbox run, a reviewer approval (second user), and a changelog note. Mirrors how `PerVideoScoringEngine.swift` consumes parameters — the Swift engine must be refactored to take a `ScoringProfile` struct decoded from the bundle instead of constants (WP-A.5).

### 4.6 Model registry
Not a training UI. It records *what model is in which app build*, where its training data came from (export job id), its licence (UTKFace research-only → "cannot ship" badge), and validation metrics uploaded from `tools/asd_age_model/validate.py`. Prevents the "no weights, no provenance" situation from repeating.

## 5. Non-functional

- Mobile-friendly (clinicians on iPad). Tables collapse to cards < 768 px.
- All lists virtualised; p95 page load < 1.5 s on 10k sessions.
- Accessibility: keyboard-complete, WCAG AA contrast.
- No PHI in URLs (ids are opaque), no analytics beyond self-hosted page counts.
- Playwright e2e for: login, session detail, review action, publish bundle (with sandbox gate), export job.

## 6. Phasing (maps to WP-C.*)

| Phase | Scope |
|---|---|
| **C0 (demo, by 2026-11-10)** | login, overview KPIs, sessions table + detail (Clips, Quality, Report tabs), review queue basic, devices enrol code |
| **C1 (by 2026-12-14)** | participants timeline, catalog/baselines/scoring editors (read + draft), bundles compose/sign/publish, sandbox re-score, audit page, users/roles |
| **C2 (by 2027-01-18)** | cohorts, norm-runs + promote, exports, model registry, retention/erasure UI, superadmin |
| **C3 (by 2027-02-15)** | replay heatmaps, Hindi/Nepali UI, clinic branding, offline/static export mode |
