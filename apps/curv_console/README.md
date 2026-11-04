# Curv Console

Clinic / research admin console for the ASD gaze product (WP-C.1, WP-C.2). Spec: `docs/roadmap_2026q4/03_asd_admin_panel_spec.md`;
backend contract: `docs/roadmap_2026q4/02_asd_backend_architecture.md` section 4.

Next.js 16 (App Router), TypeScript strict, zod. Plain CSS (light/dark), strings in `src/messages/en.ts`.

## Scope (C0 slice)

- WP-C.1: login / refresh / logout (httpOnly cookies), org switcher, Overview KPIs (`/orgs/{id}/metrics`), Sessions
  table (filters + cursor pagination), Session detail (Clips, Quality, Report tabs).
- WP-C.2: Review queue (`/[org]/review`: reasons, status/reason filters, SLA age, triage actions Confirm / Flag capture
  issue / Exclude from norms / Dismiss, keyboard j/k, 1-4, o) and Devices (`/[org]/devices`: list, revoke, generate an
  enrol code shown as text and QR).

Editors, participants, users, audit etc. arrive in later WP-C packages.

## Run

```bash
cd apps/curv_console
npm install
cp .env.example .env.local     # CURV_API_MODE=mock by default
npm run dev                    # http://localhost:3000
```

Mock mode needs no backend: any email with password `demo` signs in to a synthetic "Demo Clinic" (64 sessions, a
review queue, 5 devices). The email's local part picks the role, to try role-gated UI: `viewer@x` (read-only queue,
no devices), `clinician@x` (no revoke), `superadmin@x` (no home org: lands on the org picker). Anything else is `admin`.
Mock state (triage, revokes) lives in server memory and resets on restart.

### Automatic logoff (WP-C.21, HIP-2)

`src/proxy.ts` signs a user out on a document load after 30 minutes without one (idle) or 12 hours after sign-in
(absolute); both are constants in `src/lib/auth/session-policy.ts`. Two httpOnly cookies carry the stamps:
`curv_login_at` (set at sign-in only) and `curv_last_seen` (refreshed on any non-prefetch authenticated request that is
still within policy, enforced on document loads only). Past either limit the auth cookies are cleared and the user lands on `/login?reason=idle|expired`. A
session without stamps (signed in before this policy) is treated as expired. The refresh and role/org cookies live
at most 12 hours. The backend is unchanged.

### Live mode

```bash
cp rust/.env.example rust/.env                                  # from the repo root; set ASD_JWT_SIGNING_KEY
docker compose -f rust/docker-compose.yml up -d --build        # API on :8000
docker compose -f rust/docker-compose.yml run --rm api admin create-org --name "Demo Clinic" --kind clinic
ASD_ADMIN_PASSWORD=... docker compose -f rust/docker-compose.yml run --rm -e ASD_ADMIN_PASSWORD api \
  admin create-user --email owner@example.org --role owner --org <org-id>
cd apps/curv_console && CURV_API_MODE=live CURV_API_BASE_URL=http://localhost:8000 npm run dev
```

The console calls the backend only from the server (server components, server actions, `proxy.ts`), so the backend
needs no CORS for it.

| Command | What |
|---|---|
| `npm test` | vitest (client, mock fixtures, auth guard, components) |
| `npm run lint` | eslint (next core-web-vitals + typescript) |
| `npm run typecheck` | `tsc --noEmit` |
| `npm run build` | production build; `postbuild` fails it if any prerendered HTML ships a script without the CSP nonce |
| `npm run e2e` | Playwright smoke in mock mode (`npm run e2e:install` once for Chromium): project `chromium` runs every spec on `next dev` (:3100); project `prod` runs the CSP specs on `next build` + `next start` (:3200), the only place the production policy (no `'unsafe-eval'`, HSTS) is exercised. `E2E_PROD_REUSE_BUILD=1` skips the rebuild while iterating |

Live e2e against a running console (the same specs; data-dependent steps skip when the org has no data):

```bash
E2E_BASE_URL=http://localhost:3000 E2E_EMAIL=owner@example.org E2E_PASSWORD=... \
  E2E_SUPERADMIN_EMAIL=root@example.org E2E_SUPERADMIN_PASSWORD=... E2E_ORG_ID=<org-id> npm run e2e
```

The live run mutates data (it confirms/flags review items, creates an enrol code and revokes a device): use a test
org.

Record workspace and share-a-case against the real backend (`e2e/live/record.live.ts`, `e2e/live/share.live.ts`):
`e2e/live/run-record-live.sh` builds `asd-backend`, creates a scratch database on `PG_URL`, starts `serve` + `worker`,
an org, owner, clinician and an enrolled device, then the console in live mode, and runs both specs (participant + QR,
visit, session link, files up to 10 MiB round trip, search, follow-ups, TOTP reset; case package, share link, public
download once). The worker writes the case ZIP and the API serves it, so the blob store must be shared: with
`ASD_BLOB_BACKEND=s3` (+ `S3_*` / `AWS_*`, e.g. MinIO) that is used, otherwise `e2e/live/fake-s3.mjs`, a small
in-memory S3 stand-in (dev tooling only). `SERVE_ONLY=1` boots everything and prints the e2e environment instead.

```bash
PG_URL=postgres://postgres@127.0.0.1:5432 apps/curv_console/e2e/live/run-record-live.sh
```

Researcher toolkit against the real backend (`e2e/live/research.live.ts`, phase 8): `e2e/live/run-research-live.sh`
is the same runner with `SUITE=research`. `e2e/live/seed-research.sh` adds a researcher, two superadmins (the four-eyes
rule), publishes the seeded config drafts, ingests 14 research-consented sessions at 30 months (each with a research
frame log from `asd-cli gaze synth`, clips renamed to catalog clips and uploaded through
`PUT /v1/sessions/{id}/blobs/research_frames`), 3 at 54 months (a band below `small_cell_k`) and 2 without research
consent. The spec (about 2 minutes) creates an experiment from the published profile, moves knobs and checks the
preview at score and frames level, previews and saves a cohort (suppressed band shown as `n < 12`), runs
`cohort_rescore`, a one-axis `param_sweep` and `age_profile`, exports `analysis_results` from the pre-filled form and
validates the download, promotes the experiment, checks that publishing an unanalysed draft is refused
(`config-unanalysed`) while the promoted one publishes, that a superadmin cannot start runs, and that a run watched
for a minute writes no `analysis.read` row before the cancel button cancels it. It needs `asd-cli` as well as
`asd-backend` (built unless `SKIP_BUILD=1`; `LIVE_BIN=dir` takes both from elsewhere) and the wasm build
(`scripts/build_asd_wasm.sh`; the runner copies it into `public/`). The spec is re-runnable on the same database
(`SERVE_ONLY=1`, then `npx playwright test -c e2e/live/playwright.live.config.ts research.live.ts` with the printed
environment).

```bash
PG_URL=postgres://postgres@127.0.0.1:5432 apps/curv_console/e2e/live/run-research-live.sh
```

CI: `.github/workflows/console.yml`.

## Architecture

- `src/lib/api/` is the only place that talks to the backend. `types.ts` holds zod schemas for every response (contract
  drift fails loudly), `live.ts` the fetch client, `mock.ts` deterministic fixtures, `index.ts` picks one by
  `CURV_API_MODE`. `report.ts` proxies the binary PDF.
- Auth: login is a server action calling `POST /v1/auth/login`. Tokens are stored only in httpOnly, SameSite=Lax cookies
  (`Secure` in production or when `CURV_COOKIE_SECURE=true`). The access cookie lifetime equals `expires_in`; when it
  lapses `src/proxy.ts` calls `POST /v1/auth/refresh` with the refresh cookie and rewrites the cookies (rotation).
  Tokens never reach client JS. The report PDF is streamed through `/api/report/[id]`.
- CSP (WP-C.16): `src/proxy.ts` sets a per-request nonce policy (`'strict-dynamic'`) on every response, redirects
  included, plus HSTS in production. Every page renders per request (`connection()` in the root layout) so Next can
  stamp the nonce. zod is imported only through `src/lib/zod.ts` (ESLint enforces it), which sets `jitless` so zod
  never probes `new Function` under the policy.
- URLs carry only opaque ids; filter state lives in the query string (the filter form is a plain GET form).
- Mutations are server actions (`src/app/[org]/review/actions.ts`, `src/app/[org]/devices/actions.ts`) that call the
  typed client with the caller's token and revalidate the affected pages.
- Role: the login response's role is kept in an httpOnly `curv_role` cookie and used only to hide controls
  (`src/lib/permissions.ts` mirrors the backend RBAC matrix); the backend enforces RBAC on every call.
- Org switcher: the backend has no "list my orgs" endpoint, so the switcher offers the current org, the home org and
  recently visited orgs (`curv_recent_orgs`, ids only, kept by `proxy.ts`) that the caller can still read. The
  superadmin (no home org) gets an org picker on `/` and an "Organisation id" field in the switcher.
- Triage (`src/lib/review.ts`) maps each action to one `PATCH /v1/review/{id}`:

  | Action (key) | `status` | `decision` | `labels` |
  |---|---|---|---|
  | Confirm (1) | `done` | `{action: "confirm"}` | `triage=confirm` |
  | Flag capture issue (2) | `done` | `{action: "capture_issue", capture_issue: <label>}` | `triage=capture_issue`, `capture_issue=<label>` |
  | Exclude from norms (3) | `done` | `{action: "exclude_from_norms", exclude_from_norms: true}` | `triage=exclude_from_norms`, `exclude_from_norms=true` |
  | Dismiss (4) | `dismissed` | `{action: "dismiss"}` | `triage=dismiss` |

  Capture-issue labels: `child_distracted`, `adult_in_frame`, `calibration_drift`, `poor_lighting`, `other`. The
  norm-aggregation job reads `exclude_from_norms` from either the label or the decision.
- Enrol codes are shown once (text + QR of the plain code) and never stored by the console. The QR is drawn as an
  inline SVG from the `qrcode` library's module matrix.

## Rust core in the browser (WP-C.5)

The baselines and scoring-profile editors validate with, and the sandbox re-scores with, the Rust core compiled to
WebAssembly (`rust/crates/asd-wasm`, built by `scripts/build_asd_wasm.sh` into `build/asd/wasm`). The console never
re-implements a scoring or validation rule. `scripts/prepare-asd-wasm.mjs` copies the build into `public/asd-wasm/`
(git-ignored); it runs as `predev`, `prebuild` and `pretest`, and before the Playwright dev server. When the build is
missing it runs `build_asd_wasm.sh` if `wasm-pack` is installed, otherwise it warns and the pages show "Rust core
unavailable" (everything else, mock mode included, works; `ASD_WASM_REQUIRED=1` makes it fail instead, for releases).
The browser imports `/asd-wasm/asd_wasm.js` at runtime (not bundled); `src/lib/asd/core.ts` turns a wasm abort into a
`panic` envelope and loads a fresh instance. The unit tests load the same files in Node (`tests/asdCoreNode.ts`) and
skip the core-backed cases when the build is absent.

Pages: `/[org]/config/baselines`, `/[org]/config/scoring` (version history, view, diff, new draft, approve / publish /
retire for superadmins, four-eyes on the server), `/[org]/config/sandbox` (re-score sessions with draft vs published
documents, no writes) and `/[org]/config/bundles` (compose and sign from the published documents: owners, admins,
superadmins). Publishing a baselines or scoring-profile version asks for a sandbox run of that version in the same
browser first, and every draft needs a changelog note: both are console rules, the backend does not enforce them.

## API contract (WP-C.8)

The contract is the Rust `asd-contract::api_v1` types, exported as `docs/spec/schemas/asd/api-v1.json`. The zod schemas
in `src/lib/api/types.ts` mirror it and `src/lib/api/contract.test.ts` fails CI when they drift (published `examples`,
enums, required properties; the mock client's data is validated too).

- `POST /v1/auth/login {email,password}` and `POST /v1/auth/refresh {refresh_token}` return
  `{access_token, refresh_token, expires_in, user:{id,email,role,org_id}}`; `POST /v1/auth/logout {refresh_token}`.
- Org-scoped: `GET /v1/orgs/{id}`, `/metrics`, `/sessions?flag&quality&from&to&bundle_version&reviewed&cursor&limit`
  returning `{items, next_cursor, total?}` (opaque cursor, limit <= 100). Session-scoped: `GET /v1/sessions/{id}`,
  `/clips` returning `{items}` where each clip carries `rows[]` (metric rows), `status` may be null, and
  `/report.pdf` (404 until a device-uploaded report exists).
- Review: `GET /v1/orgs/{id}/review?status&reason&cursor&limit` -> `ReviewPage`, `PATCH /v1/review/{id}`
  `UpdateReviewRequest {status?, assigned_to?, decision?, labels?}` -> `ReviewItem`. Devices: `GET /v1/orgs/{id}/devices`
  -> `DevicePage`, `POST /v1/devices/{id}/revoke` -> `Device`, `POST /v1/orgs/{id}/devices/enrol-code`
  `EnrolCodeRequest` -> `EnrolCodeResponse`.
- Flag is `green|yellow|red|unreliable`; `battery` is a catalog protocol id string; `capture_source` is
  `truedepth|simulated|synthetic`; participants are shown by `participant_token`.
- Errors are RFC 7807 problem+json (`type` = `https://schemas.curv.org/asd/problems/<slug>`, optional `invalid_params`).

## Licences

All direct dependencies are MIT/Apache-2.0 (`qrcode` MIT, `@playwright/test` Apache-2.0). Transitive notes for `docs/license_policy.md` review: Next.js optionally
pulls `@img/sharp-libvips-*` (LGPL-3.0, dynamically loaded); this app sets `images.unoptimized` and does not use
`next/image`, so it is not exercised, but add `--omit=optional` to production installs if a strict reading is wanted.
`caniuse-lite` is CC-BY-4.0 data (build-time only).

Fonts: **Noto Sans Devanagari** (Noto Project Authors, SIL Open Font License 1.1; licence text in `src/app/fonts/OFL.txt`),
variable weight, two `woff2` subsets (Devanagari, Latin) taken from the `@fontsource-variable/noto-sans-devanagari` 5.3.0
package and self-hosted through `next/font/local`; no runtime request to a font CDN, CSP `font-src 'self'` unchanged. It
is the default pending owner ratification (ADR-0013 Q7).

## Localisation (WP-L.1.0)

The console ships `en` and `ne` (ADR-0013). Plumbing only so far; the wave packages translate the pages.

- **Locale:** `resolveLocale()` (`src/lib/i18n/server.ts`) is the signed-in user's `effective_locale` from `GET /users/me`
  (user > org > `Accept-Language` > `en`, decided by the backend); without a session (login, `/share/*`) it is the
  `Accept-Language` match (`src/lib/i18n/negotiate.ts`, tested against `docs/spec/schemas/asd/i18n-negotiation-cases.json`), else `en`.
  The root layout sets `<html lang>` (`en` or `ne-NP`), the title and the font, and wraps the tree in `LocaleProvider`.
- **Text:** server components `const t = await getText()`, client components `const t = useText()` (`src/lib/i18n`).
  Every `src/messages/<file>.ts` exports `<file>Messages = { en, ne }`; `ne` is typed by `en` (`Widen`), so a missing key
  is a compile error. `ne` is a `// ne: TODO` copy until the owning wave package translates it and sets `NE_PENDING = false`
  in `tests/messages/<file>.test.ts`. The old named exports (`en`, `adminText`, ...) stay as aliases of the `en` trees.
- **Domain labels** (enums, flag text, problem sentences, parameter labels) come from the generated `src/messages/domain.ts`
  (`asd-cli i18n export --target console`), never edited by hand. `paramText(locale, spec)` and `problemText(t, problem)` read it.
- **Formatting:** `src/lib/format.ts` takes a locale (default `en`); digits are Latin and the calendar Gregorian in both locales.
- **Mock mode:** a `+ne` tag on the email's local part (`clinician+ne@example.org`) gives that user the locale `ne`;
  `PATCH /users/me` stores a locale per mock user; an untagged user follows the mock org's `default_locale`, then `Accept-Language`.
- **Lint:** `react/jsx-no-literals` and `curv/no-literal-ui-props` (`eslint-rules/`) run at `warn`; add a directory to
  `LITERAL_GATE_ERROR_DIRS` in `eslint.config.mjs` once it is clean. Exempt a line with
  `// eslint-disable-next-line react/jsx-no-literals -- i18n-ok: <reason>`.

## Backend gaps (reported, not worked around in the backend)

- No `GET /v1/orgs` (or "my orgs") endpoint: the org switcher can only offer recent orgs and a free id field.
- `ReviewItem` carries only `session_id`: the queue reads each session (`GET /v1/sessions/{id}`) for its flag,
  quality and participant token, one call per row. Embedding a session summary in `ReviewItem` would remove that.
- `ReviewItem` and `Device` have no published `examples` in `api-v1.json` (the contract test uses fixtures instead).
- `decision` and label keys/values are free JSON in the contract; the triage vocabulary above is a console convention
  that only `exclude_from_norms` is read by the backend.
- The QR payload is the plain enrol code; the iOS app has no QR scanner yet and no URL format is defined.
- WP-C.5 sandbox: re-score uses the stored record from `GET /v1/sessions/{id}/raw` (asd-wasm `rescoreSession`, schema
  1.6 to 1.8, per-clip valid-sample ratios as recorded). Gaze frames are encrypted on the device and not served, so
  segment windows are not recomputed. Sessions carry no age; the sandbox asks for it. The sandbox-before-publish gate
  and the required changelog note are not enforced by the backend.
- WP-C.14 participant workspace: timeline `kind`/`from`/`to` filters, `GET /orgs/{id}/sessions?participant_id=` and
  `GET /participants/{id}/summary` (care counts for the print view; owner and clinician, audited) are served by the
  backend; the console no longer filters a page client-side or scrapes the timeline.

## Not yet done

shadcn/ui + TanStack Query + Recharts (the spec stack), virtualised lists, review assignment and bulk actions,
the session Review/Audit tabs, CI e2e job (`.github/workflows/console.yml` is outside this package).
