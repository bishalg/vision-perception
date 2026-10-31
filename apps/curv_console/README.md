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
| `npm run build` | production build |
| `npm run e2e` | Playwright smoke (starts `next dev` on :3100 in mock mode; `npm run e2e:install` once for Chromium) |

Live e2e against a running console (the same specs; data-dependent steps skip when the org has no data):

```bash
E2E_BASE_URL=http://localhost:3000 E2E_EMAIL=owner@example.org E2E_PASSWORD=... \
  E2E_SUPERADMIN_EMAIL=root@example.org E2E_SUPERADMIN_PASSWORD=... E2E_ORG_ID=<org-id> npm run e2e
```

The live run mutates data (it confirms/flags review items, creates an enrol code and revokes a device): use a test
org.

CI: `.github/workflows/console.yml`.

## Architecture

- `src/lib/api/` is the only place that talks to the backend. `types.ts` holds zod schemas for every response (contract
  drift fails loudly), `live.ts` the fetch client, `mock.ts` deterministic fixtures, `index.ts` picks one by
  `CURV_API_MODE`. `report.ts` proxies the binary PDF.
- Auth: login is a server action calling `POST /v1/auth/login`. Tokens are stored only in httpOnly, SameSite=Lax cookies
  (`Secure` in production or when `CURV_COOKIE_SECURE=true`). The access cookie lifetime equals `expires_in`; when it
  lapses `src/proxy.ts` calls `POST /v1/auth/refresh` with the refresh cookie and rewrites the cookies (rotation).
  Tokens never reach client JS. The report PDF is streamed through `/api/report/[id]`.
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

## Backend gaps (reported, not worked around in the backend)

- No `GET /v1/orgs` (or "my orgs") endpoint: the org switcher can only offer recent orgs and a free id field.
- `ReviewItem` carries only `session_id`: the queue reads each session (`GET /v1/sessions/{id}`) for its flag,
  quality and participant token, one call per row. Embedding a session summary in `ReviewItem` would remove that.
- `ReviewItem` and `Device` have no published `examples` in `api-v1.json` (the contract test uses fixtures instead).
- `decision` and label keys/values are free JSON in the contract; the triage vocabulary above is a console convention
  that only `exclude_from_norms` is read by the backend.
- The QR payload is the plain enrol code; the iOS app has no QR scanner yet and no URL format is defined.

## Not yet done

shadcn/ui + TanStack Query + Recharts (the spec stack), virtualised lists, review assignment and bulk actions,
the session Review/Audit tabs, CI e2e job (`.github/workflows/console.yml` is outside this package).
