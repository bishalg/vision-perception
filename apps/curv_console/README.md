# Curv Console

Clinic / research admin console for the ASD gaze product (WP-C.1). Spec: `docs/roadmap_2026q4/03_asd_admin_panel_spec.md`;
backend contract: `docs/roadmap_2026q4/02_asd_backend_architecture.md` section 4.

Next.js 16 (App Router), TypeScript strict, zod. Plain CSS (light/dark), strings in `src/messages/en.ts`.

## Scope (C0 slice)

Login, authenticated layout and nav, Overview KPIs, Sessions table (filters + cursor pagination), Session detail
(Clips, Quality, Report tabs). Review queue, devices, editors etc. arrive in later WP-C packages.

## Run

```bash
cd apps/curv_console
npm install
cp .env.example .env.local     # CURV_API_MODE=mock by default
npm run dev                    # http://localhost:3000
```

Mock mode needs no backend: any email with password `demo` signs in to a synthetic "Demo Clinic" (64 sessions).
For the real backend set `CURV_API_MODE=live` and `CURV_API_BASE_URL=https://...` (server-side only).

| Command | What |
|---|---|
| `npm test` | vitest (client, mock fixtures, auth guard, components) |
| `npm run lint` | eslint (next core-web-vitals + typescript) |
| `npm run typecheck` | `tsc --noEmit` |
| `npm run build` | production build |

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
- Flag is `green|yellow|red|unreliable`; `battery` is a catalog protocol id string; `capture_source` is
  `truedepth|simulated|synthetic`; participants are shown by `participant_token`.
- Errors are RFC 7807 problem+json (`type` = `https://schemas.curv.org/asd/problems/<slug>`, optional `invalid_params`).

## Licences

All direct dependencies are MIT/Apache-2.0. Transitive notes for `docs/license_policy.md` review: Next.js optionally
pulls `@img/sharp-libvips-*` (LGPL-3.0, dynamically loaded); this app sets `images.unoptimized` and does not use
`next/image`, so it is not exercised, but add `--omit=optional` to production installs if a strict reading is wanted.
`caniuse-lite` is CC-BY-4.0 data (build-time only).

## Not yet done

Playwright e2e smoke, shadcn/ui + TanStack Query + Recharts (the spec stack), org switcher, virtualised lists.
