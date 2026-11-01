# ADR-0009: Phase 5 security operations: TOTP enrolment, KEK rotation, plaintext column drop, metrics exposure, console CSP/HSTS, housekeeping

Status: Proposed, 2026-11-01
Spec ids: AUTH-6..AUTH-10, KEY-8..KEY-12, OBS-2..OBS-4, HK-1..HK-5, CSP-1..CSP-4, CFG-3 in
`docs/spec/modules/asd-rust-core-and-backend/technical.md` section 8
Packages: WP-B.11..B.15, WP-C.15, WP-C.16, WP-A.13, WP-G.8 (tracker `04 §7`)

## Context

Phases 1 to 4 (the last four commits on `dev`) left the backend with four security
follow-ups that the 2026-10-31 audit and the WP-E.6 security pack name explicitly
(`docs/security/threat-model.md` R1, R2, R7, F1..F4, F7; `retention-policy.md` G4, G5, G11;
`aws-baa-checklist.md` "KEK rotation", "TOTP enrolment", "`/metrics` must not be public"):

1. Login enforces TOTP for enrolled users and seeds are sealed under `ASD_DATA_KEK`
   (migration 0016), but nothing lets a user enrol. `users.totp_secret_ct` is written only
   by `auth::totp::set_secret`, which has no caller except the 0016 back-fill.
2. The plaintext `users.totp_secret` and `review_items.note` columns still exist; both are
   emptied by back-fills (`asd-backend migrate`, `admin encrypt-review-notes`) and promised
   a drop "in a later migration".
3. `Keyring` knows exactly one KEK. Rotating `ASD_DATA_KEK` today means every
   `org_data_keys` unwrap and every TOTP check fails with `503` until the data is rewritten,
   and no tool rewrites it.
4. `idempotency_keys` (ADR-B5 promises a 7-day TTL), expired `refresh_tokens` and
   `enrol_codes`, and finished `jobs` rows are never deleted.
5. 02 §4 promises a Prometheus `GET /metrics`; the code has none (`routes/metrics.rs` is the
   per-org console dashboard). The BAA checklist requires that if it is added it is not
   public.
6. The console sets `X-Frame-Options`, `X-Content-Type-Options` and `Referrer-Policy`
   (`apps/curv_console/next.config.ts`) but no CSP and no HSTS; the API sets no HSTS.

What exists and is reused (verified at `cb0df4c`): the RFC 6238 check with replay guard
(`auth/totp.rs`, `clinical/reauth.rs`: HMAC-SHA-1, 6 digits, 30 s, one step of skew);
`Keyring::seal_user_secret` / `open_user_secret` with a KEK-derived key and `kek_id`
diagnosis; the phased job runner with `run_after`, heartbeat, reclaim, `FOR UPDATE SKIP
LOCKED` and the once-a-day scheduling pattern of `jobs::retention::schedule_nightly`
(advisory lock plus "queued in the last day" check); PHI-safe request spans that record the
matched route template (`app::router`); the console's `proxy.ts` (runs on every page
request) and `next.config.ts` `headers()`; the `qrcode` dependency and `QrCode.tsx` (inline
SVG, no `data:` URL) already used for enrol codes.

Two facts the security pack gets wrong, found while reading the code:

- `integration_destinations.secret_ct` is sealed with the **org data key** (`interop/
  destination.rs`, AAD `integration_destinations:secret_ct:{org}:{id}`, column
  `secret_key_id`), not with the KEK. KEK rotation therefore touches exactly two things:
  `org_data_keys.wrapped_dek` and `users.totp_secret_ct` (plus the pending-seed column this
  ADR adds). Threat model R1/F1 and the BAA checklist list a third table that does not need
  it.
- `jobs` rows are referenced by `norm_runs.job_id`, `exports.job_id`,
  `export_deliveries.job_id` and `erasure_requests.job_id` (0017), all plain `REFERENCES`
  with no `ON DELETE`. A naive "delete finished jobs" sweep fails on the foreign key.

## Decisions

1. **TOTP enrolment is self-service, password-confirmed, two-step, under `/v1/users/me/totp`.**
   `POST .../enrol {password}` generates a 20-byte random seed, stores it sealed as a
   *pending* seed (new columns `totp_pending_ct`, `totp_pending_kek_id`,
   `totp_pending_expires_at`, migration 0020) and returns the base32 seed and an
   `otpauth://` URL once. `POST .../confirm {code}` moves the pending seed to
   `totp_secret_ct` when the code matches, spending that step (`last_totp_step`).
   `DELETE /v1/users/me/totp {password, code}` disables it. A pending seed never affects
   login: until confirmed the user is "not enrolled". The password is re-checked on enrol
   and disable so a stolen console session cannot add or remove a second factor; the
   current code is required on disable for the same reason. There is no org-admin "reset
   another user's TOTP" endpoint in this phase; recovery is the operator command
   `asd-backend admin reset-totp --user <id>`, audited. Making TOTP mandatory per role is
   owner decision O-10 and stays out; the console shows a nudge for privileged roles.
2. **The console enrolment page renders the QR in the browser from the `otpauth` URL and
   never keeps the seed after the confirm step.** The seed and the password go through a
   server action straight to the API (the existing sign-dialog pattern: never in React
   state); the page shows the seed as text for manual entry while the enrolment is pending
   and drops it on success or navigation.
3. **The Keyring reads under several KEKs and writes under one.** `ASD_DATA_KEK` stays the
   active key; new optional `ASD_DATA_KEK_PREVIOUS` (comma-separated) lists keys that may
   still unwrap or open values whose stored `kek_id` matches. Selection is by `kek_id`, so a
   value under an unknown KEK is still `503` and logged with both fingerprints, as today.
   Writes (new DEK wrap, `set_secret`, enrolment) always use the active KEK.
4. **KEK rotation is an offline, idempotent, compare-and-set rewrite: `asd-backend admin
   rotate-kek`.** It runs with the new key active and the old key in `PREVIOUS`, re-wraps
   each `org_data_keys` row and re-seals each sealed user secret whose `kek_id` is not the
   active one, each update guarded by `WHERE kek_id = <old>` so a concurrent writer on the
   new key is never overwritten, in bounded batches, and ends with the count still under
   foreign KEKs (non-zero exit when any remains). DEK values, AADs and the blind-index key
   (derived from the first DEK) do not change, so no clinical envelope is touched. The
   runbook order is: deploy `ASD_DATA_KEK=new` plus `ASD_DATA_KEK_PREVIOUS=old` to API and
   worker, run `rotate-kek`, verify zero remaining, remove `PREVIOUS`, redeploy. The old
   KEK is kept in the secret store until the last step. DEK rotation (threat model F2) is
   not in this ADR.
5. **Plaintext columns are dropped by migration 0022 behind a guard, and `migrate` runs the
   0016 back-fill before applying new migrations.** 0022 raises (`RAISE EXCEPTION`) when any
   `users.totp_secret` or `review_items.note` is still non-NULL, naming the command to run;
   it then drops both columns, the partial index `review_items_plain_note_idx` and the two
   0016 check constraints. Because `asd-backend migrate` today applies every pending
   migration *before* the KEK back-fill, a database stuck between 0016 and 0022 with a
   plaintext seed would never get past the guard; so `migrate` gains a pre-step: when 0016
   is applied, `users.totp_secret` exists and plaintext rows remain, it runs
   `encrypt_legacy_secrets` first (refusing without the KEK, as today) and only then applies
   0020 onward. Review notes are not back-filled automatically (`encrypt-review-notes` is
   the operator's explicit step, KEY-7); the guard message names it. After 0022 the legacy
   branches in `totp::load_secret`, `legacy_secret_count`, the `OR totp_secret IS NOT NULL`
   in `/users/me`, `db::backfill` and the `encrypt-review-notes` command are deleted, since
   `sqlx` compile-time checks would fail on the missing columns anyway.
6. **Housekeeping is one daily phased job, `housekeeping_sweep`, scheduled like the
   retention sweep.** Rules: `idempotency_keys` older than 7 days (ADR-B5, every scope);
   `refresh_tokens` expired more than 1 day ago (rotated-out rows stay until their own
   expiry so family-reuse detection keeps working for the token's lifetime); `enrol_codes`
   expired or revoked more than 90 days ago (the maximum lifetime) **and** not referenced by
   any device (CON-11 rotation looks the code up by hash even when expired, so a referenced
   code must stay); `jobs` in a final state with `finished_at` older than 30 days **and**
   not referenced by `norm_runs`, `exports`, `export_deliveries` or `erasure_requests`.
   Batched deletes, counts in the job result, one `housekeeping.sweep` audit row (org NULL)
   per run that deleted anything, `dry_run` param. The 30-day job retention is a constant,
   flagged for the owner, not a setting.
7. **Prometheus metrics are served on a separate listener, never on the API router.**
   `ASD_METRICS_BIND_ADDR` (unset = off) binds a second Axum router with only `GET /metrics`
   (text exposition 0.0.4), no auth, no CORS, intended for a private interface or the task's
   sidecar network. Labels come only from the matched route template, the method and the
   status class; never from the path, query, org id or user. The initial set is HTTP
   request count and duration by route, rate-limit rejections by limiter, job rows by kind
   and status (one grouped query per scrape, cached 10 s) and `asd_backend_build_info`. The
   worker process exposes nothing in this phase. This satisfies the BAA checklist ("not
   public") without inventing an auth scheme for scrapers; WP-B.7 wires the interface.
8. **The API sends HSTS unconditionally; the console sets a nonce-based CSP in `proxy.ts`
   and HSTS in `next.config.ts`.** `Strict-Transport-Security: max-age=63072000;
   includeSubDomains` on every API and console response (browsers ignore it over plain
   HTTP, so local development is unaffected); no `preload` until the domain is final
   (WP-E.2). CSP: `default-src 'self'; script-src 'self' 'nonce-<n>' 'wasm-unsafe-eval'
   'strict-dynamic'; style-src 'self' 'unsafe-inline'; img-src 'self' blob:; font-src
   'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self';
   frame-ancestors 'none'` (no `upgrade-insecure-requests`: it would break plain-HTTP local development and HSTS covers production), with `'unsafe-eval'` added only when `NODE_ENV=development`
   (dev tooling). `'wasm-unsafe-eval'` is required because the console instantiates
   `asd_wasm_bg.wasm` from `/asd-wasm/`. `style-src 'unsafe-inline'` is accepted because
   React style attributes (`DriftChart`, `LossBar`, `Distribution`) cannot carry a nonce;
   script injection is the risk that matters (R7) and scripts get the nonce. The nonce is
   generated per request in `proxy.ts` and passed to Next through the request header
   `x-nonce` so framework inline scripts carry it. `Content-Security-Policy-Report-Only` is
   not used: the policy is enforced from the first deploy and the e2e suite fails on any
   CSP violation.
9. **`ScoringProfile.trackingQuality` reaches the gaze pipeline through Rust, not Swift.**
   `asd-ffi` gains `gaze_config_json_for_profile(model_identifier, orientation,
   scoring_profile_json)` building `PipelineConfig` (device geometry, `qualityGate` and
   `params` from the profile, defaults when the profile predates 1.1.0); the kit calls it
   instead of assembling `qualityGate` JSON itself. The `scoring` coupling block
   (`maxAccuracyDegForScoring`, `aoiResolutionFactor`, `ivtVelocityDegPerSec`) travels with
   the params but nothing in `asd-core` scoring reads it until WP-G.13, so H-G2 does not
   gate this wiring; it gates only the scoring rule change. The target schedule the UI
   animates already comes from the pipeline's own config, so a delivered profile that
   changes layouts or windows changes the screens consistently.
10. **Scope of the batch.** In: the eight packages above plus WP-G.8 (the bench capture
    build is the owner's prerequisite for the blocked WP-G.14 and touches only new files
    and `ARSessionBridge`). Out, waiting for phase 6: WP-A.6 (Swift 6 mode) and WP-A.7
    (`Biomarker*` renames), because both are tree-wide changes that cannot run beside any
    other kit package and the batch is already at nine; the `research_consented` question
    (see Open questions) because it is consent semantics under DPIA O-6, not code.

## Alternatives rejected

- **TOTP enrolment without password confirmation, or admin-driven enrolment.** A stolen
  access token could then bind an attacker's authenticator; admin-set seeds would have to
  be transmitted out of band. Rejected.
- **Pending seed kept in process memory or in the access token.** Multi-replica deployments
  and the 15-minute token make both unreliable; a nullable column pair is cheaper and
  survives restarts. Rejected.
- **A reversible "previous KEK" stored in the database.** The KEK must never reach the
  database (0016 comment); `PREVIOUS` stays in the environment. Rejected.
- **Online rotation inside the API (background task).** Doubles the surface that holds the
  old key and makes "is it finished" hard to answer; the offline command with a remaining
  count is auditable. Rejected.
- **0022 silently NULLing leftover plaintext seeds.** That would un-enrol users without
  telling anyone, weakening login to password only. The guard fails loudly. Rejected.
- **Housekeeping as four cron entries or SQL `pg_cron`.** The runner already exists, is
  tested and audited; a second scheduler would need its own locking. Rejected.
- **`/metrics` on the main router behind a bearer or basic auth.** Scrapers then hold an
  API credential; a mis-scoped token leaks org data. A separate listener on a private
  interface is the Prometheus convention and needs no secret. Rejected.
- **CSP without nonces (`'unsafe-inline'` scripts) or report-only first.** Inline-script
  allowance is the XSS vector R7 names; report-only delays the protection with no user
  benefit on a console with a complete e2e suite. Rejected.
- **Merging the profile into the pipeline config in Swift.** Two lines today, but it puts a
  contract rule (which profile versions carry `trackingQuality`, which defaults apply) in
  Swift against `CLAUDE.md`. Rejected.
- **Including WP-A.6 and WP-A.7 to "finish phase 2".** See decision 10.

## Consequences

- Three migrations: 0020 (pending TOTP columns, WP-B.11), 0021 (ingest `request_hash` and
  housekeeping indexes, WP-B.12 audit fix) and 0022 (drop plaintext columns, WP-B.14). The
  tracker's migration rule moves to "≥0023".
- `asd-backend migrate` changes its ordering (pre-back-fill); the runbook in `rust/README`
  and `.env.example` must say so. `admin encrypt-review-notes` disappears with 0022; any
  deployment that still has plaintext review notes must run it **before** upgrading to a
  build containing 0022 (the guard enforces it).
- `ASD_DATA_KEK_PREVIOUS` is a new environment variable; the API and worker must both get it
  during a rotation.
- The console's `proxy.ts` now runs on every request including static pages that were
  previously skipped by the matcher; CSP must be applied uniformly, and e2e tests in mock
  mode must observe no `securitypolicyviolation`.
- `apps/curv_console` gains a security settings page; `api-v1.json` gains the TOTP DTOs
  and four problem slugs (`invalid-credentials`, `totp-already-enrolled`,
  `totp-enrolment-expired`, `totp-not-enrolled`).
- The TOTP route limiter (5/min per user) is in-process, so with N API replicas it allows
  5·N per minute. Accepted: a pending seed needs the password and lives 10 minutes.
- `asd-ffi` gains the profile-driven pipeline config and the bench raw-log encoders
  (`raw_log.rs`). The encoders are not feature-gated, so their symbols are present in
  Release/DemoRelease binaries as dormant code; the capture path (`ASD_BENCH_CAPTURE`) is
  Debug-only and is what `check_release_symbols.sh` checks. The xcframework is rebuilt by
  `scripts/ios_test.sh`.
- Not done by this ADR: mandatory TOTP per role (O-10), DEK rotation (F2), console idle
  timeout (F7 second half), worker metrics, HSTS `preload`, org-admin TOTP reset, cell-size
  rule for research exports (F8).

## Open questions for the owner

1. **O-10 (DPIA):** which roles must have TOTP, and with what grace period. Recommendation:
   owner, admin, clinician, superadmin; enforce in phase 6 by refusing login without
   enrolment after a 14-day nudge.
2. **Finished-job retention:** 30 days (decision 6). Recommendation: accept; it only affects
   unreferenced rows.
3. **HSTS `preload`:** not set. Recommendation: revisit after WP-E.2 fixes the domain.
4. **`research_consented` in exports** (phase-4 note in `04 §0`): today
   `export_build/record.rs` reads `summary.normCollection.consented`, and iOS writes
   `normCollection` only with research consent **and** the norm-collection opt-in, so a
   family that consented to research but not to norm collection is excluded from
   `research` exports. The session payload carries no other consent fact, so counting them
   in would be inference, not evidence. Recommendation: keep the conservative behaviour,
   document it in the DPIA under O-6 (consent capture design), and if the owner wants the
   broader reading add an explicit `rawPayload.consent` object to session schema 1.9 in a
   later phase. This is an owner decision because it is the meaning of the consent text.
5. **`check_asd_claims.py` extension (WP-A.7, deferred):** `CLAUDE.md` says Python tooling
   is "not extended with product logic; replace with Rust when touched". The scanner is
   dev tooling; confirm that a small extension is acceptable or that A.7 should port it to
   `asd-cli claims check` first.
