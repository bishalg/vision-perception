# Threat model: ASD screening platform

Status: draft for owner review (WP-E.6), written 2026-11-01 against branch `dev` at `e6e9326`.
Scope: iOS app, Rust backend (`asd-backend`), web console (`curv_console`), blob store, Postgres, outbound integrations.
Not in scope: Flood and Palm products, the legacy C++ core.

Path shorthand used in every table: **BE/** = `rust/crates/asd-backend/`, **EXP/** = `rust/crates/asd-export/`, **IOS/** = `apps/ios_asd_gaze_app/Sources/`, **KIT/** = `domains/asd_screening/ios/Sources/CurvAsdGazeKit/`, **CON/** = `apps/curv_console/`.

Rule for this pack: a statement about the system is backed by a file path. Anything that is **not built yet** is marked **NOT IMPLEMENTED**. Anything that needs the owner is marked **OWNER DECISION** and collected in the last section.

## 1. What we protect (assets)

| # | Asset | Sensitivity | Where it lives |
|---|---|---|---|
| A1 | Gaze session summary (scores, dwell ratios, flag, tracking quality) tied to a pseudonymous participant token | High: child health observation | Postgres `sessions.summary` (JSONB, clear), `session_clip_scores` (`BE/migrations/0001_initial.sql`); schema has no name or free text (`docs/spec/schemas/asd/session-v1.8.json`) |
| A2 | Frame-level gaze file `frames.jsonl.enc` | High | Device vault; optional upload to the blob store as an opaque blob (`BE/src/routes/blobs.rs`) |
| A3 | Hospital MRN (opt-in per org) | High: identifier | `patient_refs.mrn_ct` (AES-256-GCM) plus `mrn_bidx` blind index (`BE/migrations/0008_clinical.sql`, `BE/src/crypto/blind_index.rs`) |
| A4 | Clinical content: impressions, recommendations, notes, referral letters, amend reasons | High | `*_ct` columns, AES-256-GCM under a per-org key (`BE/migrations/0008_clinical.sql`) |
| A5 | Key material: `ASD_DATA_KEK`, per-org data keys (DEKs), JWT signing key, bundle signing key | Critical | KEK and signing keys in process environment (`BE/src/config.rs`); wrapped DEKs in `org_data_keys` (`BE/src/crypto/keyring.rs`) |
| A6 | User credentials: Argon2id password hashes, TOTP seeds, refresh tokens | Critical | `users.pw_hash`, `users.totp_secret_ct`, refresh table (hash only) (`BE/src/auth/password.rs`, `BE/src/auth/totp.rs`, `BE/src/auth/refresh.rs`) |
| A7 | Device credentials: `dvc_` tokens (hash at rest), enrol codes (hash at rest) | High | `devices.token_hash`, `enrol_codes.code_hash` (`BE/src/auth/device_token.rs`, `BE/src/auth/mod.rs`); Keychain on the phone (`IOS/Models/DeviceCredentialStore.swift`) |
| A8 | Export artifacts (CSV, XLSX, FHIR, HL7, datasets), identified or de-identified | High (identified), Medium (de-identified) | Blob store `exports/{org}/{export}/{file}` (`BE/src/jobs/export.rs`) |
| A9 | Integration secrets: webhook signing secrets, SFTP private key seeds | High | `integration_destinations.secret_ct`, sealed with the org key (`BE/src/interop/destination.rs`) |
| A10 | Audit log (actors, IPs, user agents, targets) | Medium, but integrity-critical | Postgres `audit_log`, append-only trigger (`BE/migrations/0002_auth_grants.sql`) |
| A11 | Care events (diary, tasks, medication, milestones) | High | Postgres `care_events` (`BE/migrations/0006_care_events.sql`) |
| A12 | Signed config bundles (scoring, norms, catalog) | Integrity-critical | `config_documents`, Ed25519 signature (`BE/src/config_docs/bundle.rs`) |

## 2. Actors and trust boundaries

Actors: parent or clinic staff using the iOS app; console users (owner, admin, clinician, researcher, reviewer, viewer; `BE/src/auth/rbac.rs`); the vendor `superadmin`; hospital systems receiving deliveries; an external attacker; a malicious or curious insider (including a compromised org admin).

```
 B1 internet          B2 app server            B3 data stores          B4 outbound
 iOS app ──TLS──►  asd-backend (api)  ───►  Postgres  /  blob store
 console server ─►  asd-backend (worker) ──► (S3 or MinIO)
 browser ─► console (Next.js server)                       └──► webhook / SFTP of the org
```

| Boundary | What crosses it | Control that exists |
|---|---|---|
| B1a device to API | device bearer token, session JSON, blobs | token hash lookup, schema validation, idempotency, per-device rate limit (`BE/src/auth/device_token.rs`, `BE/src/ingest/validate.rs`, `BE/src/routes/sessions.rs`) |
| B1b browser to console server | httpOnly cookies, server-action calls | cookies `httpOnly`, `sameSite=lax`, `secure` in production (`CON/src/lib/auth/cookies.ts`); frame, nosniff and referrer headers (`CON/next.config.ts`) |
| B1c console server to API | access JWT as bearer | all calls are server-side (`CON/src/lib/auth/server.ts`); no `NEXT_PUBLIC_` secrets found in `CON/src` |
| B2 API to stores | SQL, S3 calls | app-level encryption of clinical columns; DB and S3 credentials from the environment (`BE/src/config.rs`) |
| B4 worker to third parties | webhook POST, SFTP upload | SSRF guard with pinned DNS answers, https only, pinned SFTP host key (`BE/src/interop/netguard.rs`, `BE/src/interop/sftp.rs`) |

Transport security: the API (WP-B.13, OBS-4) and the console (WP-C.16, CSP-2) set HSTS themselves; TLS is terminated by Caddy in the clinic stack `deploy/clinic-server/` (WP-B.18, DPL-3: Let's Encrypt, Caddy's internal CA, or Cloudflare Tunnel), with Postgres and MinIO unpublished. The iOS app refuses non-https and IP-literal hosts (`IOS/Models/ResearchStudyConfiguration.swift`).

## 3. Existing mitigations (reference list)

| ID | Mitigation | Evidence |
|---|---|---|
| M1 | Passwords: Argon2id (m=19456 KiB, t=2, p=1); login does one verification even for unknown users, one generic 401 | `BE/src/auth/password.rs`, `BE/src/routes/auth.rs` |
| M2 | Access JWT: EdDSA only, 15 minutes, role/org/status re-read from `users` on every request, so a disabled user is cut off at once | `BE/src/auth/jwt.rs`, `BE/src/auth/extractors.rs` |
| M3 | Refresh tokens: opaque, SHA-256 at rest, single use, rotated, 30-day life; reuse revokes the whole family and is audited | `BE/src/auth/refresh.rs`, `BE/src/routes/auth.rs` |
| M4 | TOTP at login when enrolled: `401 totp-required` / `totp-invalid`, code spent per time step by compare-and-set, per-user limit 10/min | `BE/src/routes/auth.rs`, `BE/src/auth/totp.rs`, `BE/migrations/0015_security_audit_fixes.sql` |
| M5 | TOTP seeds encrypted at rest (AES-256-GCM, key derived from the KEK, AAD bound to user id); a sealed seed without a KEK fails closed (503), never "not enrolled"; plaintext seeds are converted by `asd-backend migrate` | `BE/migrations/0016_totp_secret_encryption.sql`, `BE/src/auth/totp.rs`, `BE/src/crypto/keyring.rs` |
| M6 | Device tokens: `dvc_` plus 32 random bytes, SHA-256 at rest, revocable, bound to one org; enrol codes: 60 bits, hashed, `max_uses`, expiry, revocable; register limited to 10/min per IP | `BE/src/auth/device_token.rs`, `BE/src/routes/devices.rs` |
| M7 | RBAC from one matrix; other-org resources answer 404; `superadmin` is denied every clinical-content action (notes, MRN, any export, integrations) and cannot grant roles that would give the same power | `BE/src/auth/rbac.rs` (`CLINICAL_CONTENT_ACTIONS`, `can_provision`) |
| M8 | Envelope encryption: AES-256-GCM, random nonce, AAD `table:column:org:row`; one DEK per org wrapped by the KEK; the crypto module is the only AEAD caller | `BE/src/crypto/mod.rs`, `BE/src/crypto/aead.rs`, `BE/src/crypto/keyring.rs` |
| M9 | MRN blind index: HMAC-SHA256 over normalised authority and MRN with an HKDF key from the org DEK; MRN searched by POST body only; every read audited without the MRN | `BE/src/crypto/blind_index.rs`, `BE/src/routes/patient_refs.rs` |
| M10 | Ingest validation: schema version check, JSON Schema check, capture-source policy (`simulated` always refused, `synthetic` only with `ASD_ALLOW_SYNTHETIC`), id charset, idempotency (same key and body replays, different body 409), withdrawn participants refused | `BE/src/ingest/validate.rs`, `BE/src/ingest/idempotency.rs`, `BE/src/ingest/mod.rs` |
| M11 | Blob uploads: 50 MiB cap, 15-minute pre-signed PUTs, proxy mode under the device bearer | `BE/src/blob/mod.rs`, `BE/src/routes/blobs.rs` |
| M12 | Export consent policy and de-identification live in the pure crate, not in callers; research and quality-improvement purposes are de-identified only; research needs per-session `normCollection.consented = true`; withdrawn participants always excluded; per-export random pseudonym salt never stored; every artifact watermarked | `EXP/src/policy.rs`, `EXP/src/deid.rs`, `EXP/src/watermark.rs`, `BE/src/jobs/export.rs` |
| M13 | Export download: audited, 5-minute link, never for `superadmin`, identified artifacts only for the requester or an `ExportIdentified` role; proxy-mode token is HMAC-SHA256 over export, user and expiry with a per-export secret | `BE/src/routes/exports.rs`, `BE/src/export_build/mod.rs` |
| M14 | Interop: SSRF guard (resolve once, check every address, connect to a checked address, https only for webhooks, metadata and loopback never); webhook bodies carry no clinical data; Standard-Webhooks HMAC signature; SFTP Ed25519 key, pinned host key, no trust on first use, no passwords | `BE/src/interop/netguard.rs`, `BE/src/interop/webhook.rs`, `BE/src/interop/sftp.rs` |
| M15 | Audit log: one row per mutation in the same transaction; UPDATE, DELETE, TRUNCATE refused by trigger even for the owner role | `BE/src/audit.rs`, `BE/migrations/0002_auth_grants.sql` |
| M16 | Log redaction in the formatter: sensitive field names dropped, e-mails and `child-` tokens masked, route templates logged instead of raw paths | `BE/src/telemetry.rs`, `BE/tests/log_redaction.rs` |
| M17 | Erasure job and retention sweep (see `retention-policy.md`) | `BE/src/jobs/erasure.rs`, `BE/src/jobs/retention.rs`, `BE/migrations/0017_privacy_jobs.sql` |
| M18 | Separate signing keys: the server refuses a JWT key equal to the bundle key | `BE/src/auth/mod.rs` |
| M19 | iOS: AES-GCM vault with key in Keychain (`AfterFirstUnlockThisDeviceOnly`), files written with `.completeFileProtection` and excluded from backup; device credential in Keychain; upload queue with file protection and backup exclusion; consent gate before capture; delete-all action | `KIT/SessionVaultManager.swift`, `IOS/Models/DeviceCredentialStore.swift`, `IOS/Models/ResearchKitSyncService.swift`, `IOS/Models/DataDeletionService.swift`, `docs/spec/adr/ADR-0002-consent-and-data-retention.md` |
| M20 | Console: tokens only in httpOnly cookies; export download is POST-only so prefetch cannot trigger a download; `Cache-Control: no-store` on the artifact | `CON/src/lib/auth/cookies.ts`, `CON/src/app/api/export/[id]/route.ts` |

## 4. STRIDE per component

S spoofing, T tampering, R repudiation, I information disclosure, D denial of service, E elevation of privilege. "Gap" entries feed section 5.

### 4.1 iOS app and device plane

| | Threat | Mitigation | Gap |
|---|---|---|---|
| S | Stolen or guessed enrol code registers a rogue device | M6: 60-bit code, hashed, limited uses and expiry, 10/min per IP | No App Attest; enrol code is the only proof (02 ADR-B4 named App Attest first). Register limiter is per process (R4) |
| S | Stolen device token impersonates a device | M6: revocable, hash at rest, Keychain `ThisDeviceOnly` on the phone | Token has no expiry. One token can post for any participant token of its org and can file an erasure for any participant of its org (`BE/src/routes/care.rs`) |
| T | Forged or altered session payload | M10 schema and policy checks; device is trusted for content by design | Scores come from the device; a rogue device can post plausible data into an org (data poisoning). Server recomputes only derived fields (`BE/src/ingest/derive.rs`) |
| R | Device denies posting | M15 audit row `session.ingest` per post with device id | None |
| I | Data read from a lost phone | M19 vault, file protection, Keychain, backup exclusion | If the Keychain is unavailable the vault falls back to an ephemeral key and old sessions stop decrypting (`KIT/SessionVaultManager.swift`): a data-loss risk, not a leak |
| I | Frames file readable by the server | Frames are sealed with the device vault key, which never leaves the phone | The blob is therefore opaque to everyone. See owner decision O-13 on whether to upload it at all |
| D | Upload flood | 60 posts/min per device, 50 MiB blob cap, request timeout 30 s | Limiter is per process (R4) |
| E | App sends a role or org it should not | Org comes from the token, never from the body | None found |

### 4.2 Backend API (user plane and device plane)

| | Threat | Mitigation | Gap |
|---|---|---|---|
| S | Credential stuffing, password guessing | M1, per-IP login limit 10/min, per-user TOTP limit 10/min, generic 401 | Limiters are in memory per process (R4). IP key depends on `ASD_TRUST_PROXY_HEADERS` being right: off behind a proxy means all clients share one IP; on without a proxy lets a client forge it (`BE/src/auth/extractors.rs`) |
| S | Account without second factor | M4 enforces TOTP when a seed exists | **No enrolment flow exists**: `totp::set_secret` has no caller besides operators (`BE/src/auth/totp.rs`), so in practice users are password-only. TOTP is not mandatory for any role |
| S | Refresh token theft | M3 rotation with reuse detection | None specific |
| T | Role or org escalation through user management | M7 `can_assign`, `can_provision`; org scoping returns 404 | None found; covered by `BE/tests/rbac_matrix.rs`, `BE/tests/rbac_clinical.rs` |
| T | Tampering with signed clinical records | Hash-bound signatures, immutable by trigger (ADR-0008 D3) | Not re-audited here (clinical sign-off is in `BE/src/clinical/sign.rs`) |
| R | Admin denies a change | M15 | Audit rows hold ids and changed fields, not secrets. No external copy of the audit log, so a DB superuser could still drop the table (R9) |
| I | Cross-tenant read | M7 404 on other-org ids, `Require<..>` extractors | None found |
| I | Clinical text or MRN read by operators | M7 `superadmin` and `admin` carve-outs; M8 encryption | `owner` and `clinician` can read; DB admin with the KEK can read (R1) |
| I | PHI in logs | M16 | Redaction is name-based; a new field with an unlisted name would log. Covered by test, so new fields need a test update |
| D | Heavy export or job abuse | Export row cap of 200,000 (`rust/crates/asd-contract/src/export.rs`), job queue (`BE/src/jobs/`), request timeout | No per-user export rate limit found |
| E | JWT algorithm confusion | Only EdDSA verified (`BE/src/auth/jwt.rs`) | None |

### 4.3 Key management and crypto

| | Threat | Mitigation | Gap |
|---|---|---|---|
| I | KEK leak | Not in `Debug` output (fingerprint only); AAD binding stops row swaps | KEK is a base64 environment variable (`ASD_DATA_KEK`). With KEK plus DB dump an attacker opens all clinical text and MRNs (R1) |
| D | KEK loss | Documented in `rust/.env.example` | Loss of the KEK loses all clinical text and every TOTP seed. No backup process exists yet |
| T | Key rotation | New DEK gets a new `key_id`; old rows keep decrypting (KEY-5). KEK rotation: `asd-backend admin rotate-kek` re-wraps `org_data_keys` and re-seals `users.totp_secret_ct` under the new KEK while `ASD_DATA_KEK_PREVIOUS` keeps the old one readable; compare-and-set, idempotent, audited `crypto.kek_rotated` (`BE/src/crypto/rotate.rs`, runbook `rust/README.md`). Integration-destination secrets are sealed with the org DEK, so the re-wrap covers them | No DEK rotation or re-encryption job for old DEKs (F2); rotation cadence undecided (O-9) |
| I | Crypto-shredding | Deleting `org_data_keys` rows makes an org's clinical content unrecoverable | Unwrapped keys are cached per process, so API and workers must restart for it to take effect (`BE/src/crypto/keyring.rs`). Operator action, not an API |
| I | Plaintext leftovers | 0016 and 0008 convert TOTP seeds and review notes | The plaintext `users.totp_secret` and `review_items.note` columns still exist; a drop migration is pending (`BE/migrations/0016_totp_secret_encryption.sql`, `BE/src/admin.rs`) |

### 4.4 Exports and integrations

| | Threat | Mitigation | Gap |
|---|---|---|---|
| I | Identified data leaves under a research label | M12: purpose and mode are checked at request time and again in eligibility; `exports_dataset_deidentified` DB check (`BE/migrations/0017_privacy_jobs.sql`) | De-identification keeps `age_bracket_at_enrol`, month-level dates, and measurements. Small cells could re-identify; no k-anonymity check exists |
| I | Download link leaked | 5-minute links, audited; proxy token bound to user and export | In presigned mode the console answers `303` to the pre-signed S3 URL (`CON/src/app/api/export/[id]/route.ts`), so the URL is visible to the browser for its 5 minutes. Treat as bearer |
| I | Artifact lingers | Expiry sweep deletes the blob after `expires_at` (default 72 h, 1 to 168 h) | Delivered copies (SFTP, a webhook receiver that downloaded the file) are outside our control |
| I | Identified artifact in the bucket | Only requester or `ExportIdentified` can fetch it | HL7 files with MRN sit unencrypted-by-us in the bucket until expiry; relies on bucket encryption (not set by code, see `aws-baa-checklist.md`) |
| S/T | Rogue destination (SSRF) | M14 | Org admins choose destinations; the guard is the only barrier. SFTP may reach RFC 1918 addresses by design (hospital VPN) |
| R | Disclosure accounting | `export_items` kept after artifact deletion; `GET /v1/participants/{id}/disclosures` | None |

### 4.5 Console (Next.js server and browser)

| | Threat | Mitigation | Gap |
|---|---|---|---|
| S | Session theft from the browser | httpOnly, sameSite lax, secure in production | No CSP is set (`CON/next.config.ts`); an XSS bug would still act as the user through server actions |
| T | CSRF on server actions | sameSite lax cookies, POST-only downloads; Next.js checks the `Origin` header on server actions | Not independently tested here |
| I | Secrets in the client bundle | Tokens only in httpOnly cookies; no `NEXT_PUBLIC_` variables found | The role cookie `curv_role` is readable by the server only, used just to hide buttons; the backend enforces RBAC |
| E | UI-only checks bypassed | Backend enforces every action (M7); console role cookie is cosmetic (`CON/src/lib/auth/cookies.ts`) | None |
| D | Cookie lifetime | Refresh cookie 7 days, shorter than the 30-day backend token | No idle timeout |

### 4.6 Data stores (Postgres, blob store)

| | Threat | Mitigation | Gap |
|---|---|---|---|
| I | DB dump or snapshot leak | App-level encryption for A3, A4, A6 (TOTP), A9 | A1, A11 and participant rows are clear in the DB. Encryption at rest of volumes/RDS is **NOT IMPLEMENTED** (WP-B.7) |
| I | Bucket exposure | Key layout per org; presigned-only access | Code sets no SSE headers (`BE/src/blob/s3.rs`); bucket default encryption and block-public-access must come from IaC (WP-B.7) |
| T | Audit tampering | Trigger refuses UPDATE/DELETE/TRUNCATE | Compose runs the API as the schema owner (`rust/docker-compose.yml`); the trigger still protects, but a production deployment should log in as `asd_app` (documented in `BE/migrations/0002_auth_grants.sql`) |
| D | Backup and restore | None yet | Restores can resurrect erased data (see `retention-policy.md`) |

## 5. Residual risks, ranked

Likelihood and impact are the author's judgement on the current code, for owner review.

| Rank | ID | Risk | Why it matters | Follow-up |
|---|---|---|---|---|
| 1 | R1 | Single environment-variable KEK, no backup procedure (rotation tool: `asd-backend admin rotate-kek`, WP-B.15) | One secret decrypts all clinical text, MRNs and TOTP seeds; losing it destroys them | WP-B.7: KEK in KMS or Secrets Manager with versioned backup; rotate with `rotate-kek` (re-wraps `org_data_keys`, re-seals `users.totp_secret_ct`; integration secrets are under the org DEK and need nothing) |
| 2 | R2 | No TOTP enrolment flow; TOTP not mandatory; plaintext `totp_secret` column still in the schema | Clinical and admin accounts are password-only in practice; step-up signing weakens to password | Build enrolment (QR, confirm code) using `totp::set_secret`; require TOTP for owner, admin, clinician, superadmin; add migration dropping `users.totp_secret` and `review_items.note` after back-fill |
| 3 | R3 | Erasure leaves signed clinical records (hold) and ADR-0008 Q2 is undecided | A parent's erasure request is only partly honoured; legal exposure depends on jurisdiction | OWNER DECISION O-1 |
| 4 | R4 | Rate limiters are in memory, per process | With N replicas the limit is N times larger; a restart resets it; login and enrol-code guessing are the targets (`BE/src/auth/mod.rs` says WAF rules are to cover it, DEP-3) | WP-B.7 WAF rate rules; later a shared limiter (Postgres or Redis) for login, TOTP and register |
| 5 | R5 | Core session data (A1, A11, participants) is stored in clear in Postgres; protection relies on infrastructure that is not built | Anyone with DB read access sees child health observations (pseudonymous) | WP-B.7 encrypted RDS, private subnets, least-privilege roles; owner decision on column encryption for `sessions.summary` |
| 6 | R6 | Device token is long-lived, org-wide, no App Attest | A leaked token can poison data or trigger erasures for any participant in the org | Per-device scoping of participant tokens; token expiry or rotation policy; App Attest (WP-B.2 follow-up) |
| 7 | R7 | Console has no CSP and no idle timeout; presigned export URL is passed through the browser | Raises XSS impact and link-leak exposure | Add CSP and HSTS in `CON/next.config.ts` (code change, outside this pack); keep presigned TTL at 5 minutes |
| 8 | R8 | De-identified exports keep age bracket, month dates, and fine-grained measurements | Re-identification by linkage in small cohorts | Add a minimum cell size rule for research exports; document in the DPIA |
| 9 | R9 | Audit log has no off-box copy and no retention rule | Tamper-evidence stops at the DB boundary; grows without bound; holds staff IPs and user agents | Ship audit rows to a write-once store (S3 Object Lock or CloudWatch) in WP-B.7; OWNER DECISION O-5 on retention |
| 10 | R10 | Backups and restores are undefined | Erased or expired data may come back; backups may hold PHI beyond policy | OWNER DECISION O-12; see `aws-baa-checklist.md` |

## 6. Follow-ups (engineering backlog from this review)

| # | Item | Related risk | Suggested owner package |
|---|---|---|---|
| F1 | KEK rotation tool: re-wrap DEKs and re-seal TOTP seeds (integration secrets are under DEKs, untouched); tested with two KEKs | R1 | Done: WP-B.15 (`asd-backend admin rotate-kek`, `tests/kek_rotation.rs`) |
| F2 | DEK re-encryption job (KEY-5 left it out of v1) | R1 | same |
| F3 | TOTP enrolment endpoints and console screens; enforce for privileged roles | R2 | WP-C follow-up |
| F4 | Migration dropping `users.totp_secret` and `review_items.note` | R2 | same as F3 |
| F5 | Shared rate limiter or WAF-only policy documented as the control | R4 | WP-B.7 |
| F6 | Device token expiry/rotation; per-device participant scoping; App Attest | R6 | WP-B.2 follow-up |
| F7 | CSP, HSTS and idle timeout in the console | R7 | WP-C follow-up |
| F8 | Minimum cell size for research exports | R8 | WP-I follow-up |
| F9 | Audit log export to write-once storage; retention rule | R9 | WP-B.7 |
| F10 | Run the API as `asd_app` (not the owner role) in every non-test deployment | audit integrity | Done by WP-B.18: `deploy/clinic-server/` runs `api` and `worker` as `asd_app` (`install.sh` sets its password after the first migration; `test/compose_smoke.sh` checks the API log line) |
| F11 | Confirm `ASD_TRUST_PROXY_HEADERS` per environment; add a startup warning when off behind a known proxy | R4 | Done by WP-B.18 for the clinic stack: `ASD_TRUST_PROXY_HEADERS=true` behind Caddy, which strips a client-sent `Cf-Connecting-Ip` outside tunnel mode (the startup warning is not built) |
| F12 | Pen-test or external review before any clinical pilot | all | owner |

## 7. Owner decisions raised by this document

O-1 (clinical retention vs erasure, ADR-0008 Q2), O-5 (audit retention), O-9 (KEK custody and rotation cadence), O-10 (mandatory TOTP and console session policy), O-12 (backup retention), O-13 (upload frames or not), O-14 (any break-glass access for the vendor). Full list with IDs: see `dpia-lite.md` section 11.
