# Retention policy (as built, plus open decisions)

Status: draft (WP-E.6), 2026-11-01. Part 1 describes what the code does **today**. Part 2 lists what is **not** done. Part 3 lists **owner decisions**; nothing in part 3 is policy until the owner signs it.
Path shorthand: **BE/** = `rust/crates/asd-backend/`, **EXP/** = `rust/crates/asd-export/`, **IOS/** = `apps/ios_asd_gaze_app/Sources/`, **KIT/** = `domains/asd_screening/ios/Sources/CurvAsdGazeKit/`.

## 1. What the code does today

### 1.1 Schedule

| Data | Rule | Mechanism | Evidence |
|---|---|---|---|
| Frame-level blob (`frames.jsonl.enc`) | Deleted when the session was **received** more than `organizations.retention_months` ago. Default 24; API accepts 1 to 240 | Worker queues one sweep per day for all orgs; manual sweep from the console with `dry_run` (default true) | `BE/src/jobs/retention.rs`, `BE/migrations/0001_initial.sql` (`retention_months`), `BE/src/routes/orgs.rs` (`MAX_RETENTION_MONTHS`), `BE/src/routes/privacy.rs` |
| Session summary, per-clip rows, reviews, clinical records | **Kept** after the sweep (these are the aggregate counts and the clinical record). `frames_purged_at` marks the session so it is skipped next time | same sweep | `BE/src/jobs/retention.rs`, `BE/migrations/0017_privacy_jobs.sql` |
| Export artifacts | Deleted when `expires_at` passes: default 72 hours, per-org setting clamped to 1 to 168 hours. Row becomes `expired`; `export_items` stay as the accounting of disclosures | Worker sweep (every minute, up to 500 per pass); downloads after expiry answer `410 export-expired` | `rust/crates/asd-contract/src/export.rs` (`DEFAULT_TTL_HOURS`, `MIN_TTL_HOURS`, `MAX_TTL_HOURS`), `BE/src/export_build/mod.rs` (`ttl_hours`), `BE/src/jobs/export.rs` (`sweep_expired`), `BE/src/jobs/runner.rs` |
| Download links | Pre-signed GET or proxy token valid 5 minutes; webhook download links 15 minutes; pre-signed upload URLs 15 minutes | Expiry in the signature | `BE/src/routes/exports.rs`, `BE/src/blob/mod.rs`, `BE/src/interop/webhook.rs` |
| Refresh tokens | Valid 30 days, single use | Expiry check at use; **no row cleanup** | `BE/src/auth/refresh.rs` |
| Enrol codes | Default 7-day expiry (max 90) | Expiry check at use; **no row cleanup** | `BE/src/routes/devices.rs`, `BE/migrations/0001_initial.sql` |
| Console cookies | Access cookie 15 minutes, refresh cookie 7 days | Browser expiry | `apps/curv_console/src/lib/auth/cookies.ts` |

### 1.2 Erasure (on request)

Erasure is requested by an org owner or admin from the console, or by a device through `POST /v1/participants/{token}/erasure`. Both withdraw consent immediately, so new uploads are refused (`BE/src/routes/privacy.rs`, `BE/src/routes/care.rs`). A job then runs (`BE/src/jobs/erasure.rs`).

| Deleted | Kept |
|---|---|
| Sessions of the participant that no held clinical record documents, with clips, QA review items and labels (cascade) | Participant row as a **tombstone**: id, org, token, `withdrawn_at`, `erased_at` (so replayed uploads stay refused) |
| Frames and report blobs of those sessions (recorded key and the ADR-B3 key) | The erasure request row with counts only, and its audit row |
| Their idempotency keys | `export_items` rows (disclosure accounting) |
| Care events; draft referral letters; unsigned clinical review drafts | Audit rows (append-only; hold ids, not child data) |
| MRN mapping (`patient_refs`), unless held | Everything listed under "held" below |
| Artifacts of unexpired exports that included the participant; pending deliveries fail | |
| Demographics and consent fields on the participant (set NULL) | |

**Held (ADR-0008 Q2 open).** If the participant has a signed or superseded clinical review version, any clinical note or a final letter, the request ends `held` (`hold_reason = clinical_record`, `completed_at` NULL): signed records, the sessions they document and the MRN mapping stay; everything else above is still erased. Signed content is immutable by trigger, and notes are not crypto-shredded because the data key is per org (`BE/src/jobs/erasure.rs`, `BE/src/crypto/keyring.rs`). A second request for a held participant is refused as a duplicate.

### 1.3 Crypto-shredding (org offboarding)

Deleting an org's rows in `org_data_keys` makes its encrypted columns (MRN, notes, letters, integration secrets) unrecoverable. It is a documented operator action, not an API, and cached keys disappear only after API and worker restart (`BE/src/crypto/keyring.rs`, KEY-5 in `docs/spec/modules/asd-clinician-and-interop/technical.md`). TOTP seeds use a key derived from the KEK and are not affected.

### 1.4 On the device

| Data | Rule | Evidence |
|---|---|---|
| Vault sessions, history, queues, calibration, M-CHAT, enrolment | Deleted by the in-app "delete all observation data" action; queued uploads are purged when research consent is revoked | `IOS/Models/DataDeletionService.swift`, `IOS/Models/ResearchKitStudyStore.swift`, ADR-0002 D6 |
| Care plans | Deleted only on a second, separate confirmation | `IOS/Models/DataDeletionService.swift` |
| Upload queue items | Retry with backoff, dead-letter after the last attempt | `IOS/Models/ResearchKitSyncService.swift` |
| Vault key | Keychain, device only, lost with the app or device | `KIT/SessionVaultManager.swift` |

## 2. Gaps: no rule or no automation today

| # | Data | What happens today | Risk |
|---|---|---|---|
| G1 | Session summaries, per-clip rows, participants | Kept forever unless erased | Retention is open-ended for the core health data |
| G2 | Care events | Kept forever unless erased | Same |
| G3 | `audit_log` | Never deleted (append-only trigger refuses it); holds staff IPs and user agents | Unbounded growth; personal data of staff with no end date |
| G4 | `idempotency_keys` | Index comment says "TTL sweep (7 days)" and 02 section ADR-B5 promises it, but **no job deletes these rows** (only erasure deletes them); stored responses include upload URLs with the participant token in the path | Stale data with identifiers; table growth |
| G5 | Expired refresh tokens and enrol codes, finished `jobs` rows | No cleanup | Table growth; job params hold filters and ids |
| G6 | Database backups and snapshots | Not defined (WP-B.7 not built) | Restores can bring back erased or expired data |
| G7 | Delivered or downloaded exports (SFTP drop, webhook receiver, researcher's disk) | Outside our control; only the watermark and the disclosure record remain | Recipient retention is unmanaged |
| G8 | `users` of a departed staff member | Disable only; no deletion path found | Staff personal data retained |
| G9 | Clinical records | No `clinical.retention_years` setting is enforced; held forever | See O-1 |
| G10 | `sessions.frames_blob_key` when the app never uploads the frames blob | Sweep only handles recorded and ADR-B3 keys | None, but check once O-13 is decided |
| G11 | Plaintext legacy columns `users.totp_secret`, `review_items.note` | Present, emptied by back-fill commands; drop migration pending | Possible leftovers on deployments that skipped the back-fill |

## 3. Owner decisions needed before this becomes policy

| ID | Decision | Options / default in code or ADR |
|---|---|---|
| **O-1** | What happens to signed clinical records when a parent asks for erasure (ADR-0008 Q2) | Today: hold indefinitely. ADR proposal: erase sessions and measurements now, crypto-shred notes only after the org's `clinical.retention_years`. Needs per-record keys (design change) and a stated period per jurisdiction |
| **O-3** | Retention of summaries, care events, participants; default `retention_months` (24); maximum allowed (240); minors reaching adulthood | Code default 24 months applies to frame blobs only |
| **O-4** | Default export lifetime (72 h) and the allowed range (1 to 168 h) | Confirm or tighten; shorter for identified exports? |
| **O-5** | Audit log retention (for example 6 or 7 years for health systems), and where it is archived | Needs a rule and an off-box copy before any pruning, because the table is append-only by design |
| **O-12** | Backup retention window and how erasures are replayed after a restore | For example: backups kept N days, and a runbook re-applies erasure requests after any restore (`erasure_requests` and tombstones survive in the DB) |
| **O-13** | Keep uploading `frames.jsonl.enc`? Nobody but the device can decrypt it, so retention purge is the only thing it receives | Stop uploading, or design key escrow (ADR-B3 says a separate ADR) |
| O-11 | Who answers rights requests and within what time | not code |

## 4. Proposed implementation work (not started)

1. Sweeps for G4 and G5 (idempotency keys older than 7 days, expired refresh and enrol rows, old finished jobs). Small, no owner decision needed except job retention period.
2. `clinical.retention_years` setting and enforcement, after O-1.
3. Audit archive and pruning, after O-5.
4. Retention extension for summaries, after O-3.
5. Restore runbook, after O-12 and WP-B.7.

None of this changes code in WP-E.6; each item needs its own work package in `docs/roadmap_2026q4/04_asd_phased_work_packages.md`.
