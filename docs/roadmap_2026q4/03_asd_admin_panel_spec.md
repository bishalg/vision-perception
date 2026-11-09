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

Route folders, URLs and the menu follow one tree (§3a, WP-C.23; `apps/curv_console/src/lib/paths.ts`). The flat
paths before WP-C.23 (`/[org]/sessions`, `/[org]/settings/security`, …) answer 308 to these for one release.

```
/login
/[org]                                  redirects to clinic/overview
Clinic
/[org]/clinic/overview                  KPIs: sessions (7d/30d), flag distribution, quality distribution, review queue size, devices active, config adoption
/[org]/clinic/worklist                  clinical worklist (worklist.read)
/[org]/clinic/participants              table: token, local ref, age bracket, sessions n, last flag, last session, consent version
/[org]/clinic/participants/[id]         timeline of sessions (composite over time, flags), care events summary, erasure button (admin)
/[org]/clinic/follow-ups                open follow-ups (encounter.write)
/[org]/clinic/review                    queue with reasons, assignment, SLA age; bulk actions; keyboard-driven triage
/[org]/clinic/sessions                  filters: date, flag, quality, battery, age pack, bundle version, device, reviewed?
/[org]/clinic/sessions/[id]             header (flag, composite, confidence, quality, bundle_version) · tabs: Clips (expected-vs-recorded table + per-clip deviation chart) · Quality (valid ratio, distance, pose, calibration) · Affect · Replay (heatmap/flow per clip from frames blob — Phase 2, requires decrypt path or client-side upload of a down-sampled replay) · Report (PDF viewer, FHIR JSON) · Review (labels, notes, history) · Audit
Research (analysis.read)
/[org]/research                         recent experiments, cohorts and runs
/[org]/research/experiments[/{id}|/new] experiments and their workbench
/[org]/research/cohorts[/{id}|/new]     saved filters; counts per cell (age × clip × metric) with n≥12 indicator
/[org]/research/runs[/{id}[/sessions]]  analysis runs (norm runs, sweeps) → cells table → promote to draft baselines (with provenance written into each cell)
Configuration (config.read)
/[org]/config/catalog                   versions list → editor: clips (id, title, age packs, languages, duration, AOI roles + time-coded AOI rects with a timeline editor over the clip preview), batteries (express/comprehensive ordering)
/[org]/config/baselines                 versions list → matrix editor: 10 clips × 5 brackets × metrics, cell provenance (literature / interpolated / own-norm run n=…), direction, tolerance; diff vs published
/[org]/config/scoring                   versions list → form: status bands, piecewise score breakpoints, weights per clip/metric, composite confidence gate (0.60), flag thresholds (0.65 / 0.35 / 40 %), quality gate params (distance, yaw, pitch, confidence, blink)
/[org]/config/sandbox                   "Re-score" tool: pick N sessions, pick a draft bundle, see how flags/composites would change vs published (no writes) — the key safety tool before publishing a scoring change
/[org]/config/bundles                   compose catalog + baselines + scoring → preview → sign + publish; adoption chart (which app versions pulled which bundle)
Data
/[org]/data/exports[/{id}]              dataset export jobs (parquet/jsonl/csv, with labels), deliveries, share links, download, hash (export.read)
/[org]/data/erasure                     erasure requests (participant.erase)
/[org]/data/retention                   retention months and the retention sweep (participant.erase)
/[org]/data/audit                       searchable append-only log, CSV export (audit.read)
Administration
/[org]/admin/users                      invite, roles, TOTP status, disable (user.read)
/[org]/admin/devices                    enrolled devices, app versions, last sync, revoke, generate enrol code (QR) (device.read)
/[org]/admin/integrations[/{id}]        delivery destinations (webhook, SFTP), credentials, test, delete (integration.read)
/[org]/admin/clinical                   clinical settings: consent version text, local-ref policy, org language (worklist.read or org.update)
Account (top-right menu, every signed-in user)
/[org]/account/security                 two-step sign-in (TOTP), language; target of the TOTP enforcement redirect
Unchanged
/share/[id], /switch-org, /api/**
/admin                                  superadmin (planned, not built): orgs, feature flags, system health (queue depth, job failures, DB size), config signing key rotation
```

## 3a. Navigation and route groups (owner decision 2026-11-09, WP-C.23)

Owner: the flat top bar (22 links for an admin) is not acceptable. The console gets **five divisions** in the
top bar and a **left sidebar** for the current division's pages. Route folders and URL paths follow the same
tree, so the code, the folders, the URLs and the menu say the same thing.

### Divisions, sidebar items and paths

| Division (top bar) | Sidebar items (in order) | Paths under `/[org]/` | Visible when |
| --- | --- | --- | --- |
| Clinic | Overview, Worklist, Participants, Follow-ups, Review, Sessions | `clinic/overview`, `clinic/worklist`, `clinic/participants[/{id}]`, `clinic/follow-ups`, `clinic/review`, `clinic/sessions[/{id}]` | always (items gated as today: Worklist `worklist.read`, Follow-ups `encounter.write`) |
| Research | Experiments, Cohorts, Analysis runs | `research/experiments…`, `research/cohorts…`, `research/runs…` (unchanged; `research` index stays) | `analysis.read` |
| Configuration | Catalog, Baselines, Scoring, Sandbox, Bundles | `config/catalog…`, `config/baselines…`, `config/scoring…`, `config/sandbox`, `config/bundles` (unchanged) | `config.read` |
| Data | Exports, Erasure requests, Data retention, Audit log | `data/exports[/{id}]`, `data/erasure`, `data/retention`, `data/audit` | any of `export.read`, `participant.erase`, `audit.read` |
| Administration | Users, Devices, Integrations, Clinical settings | `admin/users`, `admin/devices`, `admin/integrations[/{id}]`, `admin/clinical` | any of `user.read`, `device.read`, `integration.read`, `org.update`/`worklist.read` (clinical settings as today) |

Not a division: **Account** (top-right menu): Account security (`account/security`, every signed-in user; the TOTP
enforcement redirect targets this path), Sign out. The org switcher stays in the top bar but compact (name only;
"Organisation id" field and "Switch" only for `superadmin`, behind the same menu as today's switcher).
`/[org]` redirects to `clinic/overview`. `/share/[id]`, `/login`, `/switch-org`, `/api/**` are unchanged.

### Rules

- N-1 A division shows only if at least one of its items is visible for the role; a role with one visible division
  still sees the bar (consistency). The sidebar lists only visible items. Hiding is UI only; the backend authorises.
- N-2 Active state: division by path prefix (`/[org]/clinic`), sidebar item by prefix of its own path; detail pages
  (`clinic/sessions/{id}`) highlight their list item. Breadcrumb line under the page title: Division › Item › Detail.
- N-3 Layout: CSS grid `sidebar 240px | content`, content max-width 1280 px as today. Below 1024 px the sidebar
  becomes a drawer opened by a "Menu" button in the top bar (WP-C.22 groundwork); the top divisions collapse into
  the same drawer below 640 px. Keyboard: the drawer traps focus and closes on Escape.
- N-4 One source of truth for paths: `src/lib/paths.ts` exports `orgPaths(base)` with a function per page
  (`clinic.session(id)`, `data.export(id)`, `admin.integration(id)`, `account.security`, …). Components, server
  actions, redirects, `proxy.ts`/`guard.ts` and e2e helpers use it; no page builds `${base}/…` strings by hand. The
  existing `research` layout's sub-nav moves into the sidebar (the PRV-6 disclaimer line stays on research pages).
- N-5 `src/lib/nav.ts` returns the tree (`navTree(base, role, t): Division[]`, each with `href`, `text`, `items`);
  unit tests assert the visible tree per role (superadmin, owner, admin, clinician, reviewer, researcher, operator,
  viewer: use `ROLES` from `lib/api/types`).
- N-6 Route folders mirror the tree: `src/app/[org]/{clinic,data,admin,account}/…` are created by `git mv` of the
  existing page folders (history preserved); `research` and `config` stay. A group gets a `layout.tsx` only when it
  needs one (research keeps its disclaimer layout).
- N-7 Old URLs keep working for one release: `next.config.ts` `redirects()` maps every old path (with `:org` and
  `:path*`) to its new one with 308. e2e, unit tests, the uncommitted `scripts/site-screenshots.spec.ts` and the
  docs switch to the new paths. `settings/security` → `account/security` is included.
- N-8 Labels: new message keys `app.nav.divisions.{clinic,research,config,data,admin}`, `app.nav.account`,
  `app.nav.menu`, `app.nav.breadcrumb` in en and ne (ne draft marked in a comment as needs native review); item
  labels reuse the existing keys. No English literal in JSX (`react/jsx-no-literals` is on).
- N-9 Tablet first pass only: no visual redesign of pages in this package; typography and tokens unchanged.

### Package

| WP | Tier | Scope | Gates |
| --- | --- | --- | --- |
| WP-C.23 | senior (Opus) | N-1..N-9: `paths.ts`, `nav.ts` tree + tests, `TopBar`/`SideNav`/`AccountMenu` components, shell CSS, folder moves, redirects, link sites (54), redirects in actions (48), `proxy.ts`/`guard.ts`, e2e (102 path literals), spec 03 §3 IA block rewritten to the new tree | `npm run lint`, `npx tsc --noEmit`, `npm test`, `npm run build`, `npm run e2e` (mock), `console.yml` steps |

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
