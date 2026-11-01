# AWS deployment and BAA checklist (requirements for WP-B.7)

**STATUS: NOT IMPLEMENTED.** WP-B.7 is `todo` in `docs/roadmap_2026q4/04_asd_phased_work_packages.md`. Nothing in this file describes a running AWS environment. It lists what WP-B.7 must deliver and what the owner must decide or sign, so the deployment can later be called "BAA-ready". The product must not be described as HIPAA-compliant until a BAA is signed and an external security review exists (`docs/roadmap_2026q4/02_asd_backend_architecture.md` ADR-B8).

Path shorthand: **BE/** = `rust/crates/asd-backend/`.
Starting points already in the repo: the target architecture in `docs/spec/modules/asd-rust-core-and-backend/technical.md` (DEP-3: ECS Fargate for api and worker, RDS Postgres 16 encrypted in private subnets, S3 with SSE, versioning and block public access, Secrets Manager, ALB + ACM + WAF, CloudWatch logs); the container build in `rust/Dockerfile` (distroless, non-root user); the local stack in `rust/docker-compose.yml`; environment variables in `rust/.env.example`. Existing `aws_infra/` is the Flood/Palm CDK app (Python, `aws_infra/app.py`); the IaC language for the ASD stack is owner decision H-11 and the project rule is no new Python (`CLAUDE.md`).

## 1. Owner decisions before building

| ID | Decision | Notes |
|---|---|---|
| O-8 | Does HIPAA apply (US covered entities or business associates as customers)? If yes, sign the AWS BAA in AWS Artifact for the account that will hold the data | The BAA is accepted per AWS account (and by the organisation's management account for AWS Organizations) in AWS Artifact; confirm current process with AWS |
| O-7 | Region(s) and data residency per market | Code default is `us-east-1` (`BE/src/config.rs`); a Nepal or India customer may need a different region or in-country hosting |
| H-4, H-11 | Hosting phase and IaC language | Tracker items |
| O-9 | KEK custody design (section 4) and rotation cadence | |
| O-12 | Backup retention and restore handling of erasures | See `retention-policy.md` |
| O-11 | Incident and breach notification contacts and timelines | HIPAA breach rules and GDPR both set deadlines |
| O-2b | Controller versus processor (HIPAA: covered entity versus business associate) per customer | A BAA chain is also needed between us and each US clinic customer |

## 2. Account, BAA and service scope

- [ ] Dedicated AWS account (or accounts: prod, staging) for PHI workloads; PHI never in a sandbox or personal account.
- [ ] BAA accepted for that account in AWS Artifact (O-8). Record date and signer in this file.
- [ ] Use **only** services on AWS's current "HIPAA Eligible Services Reference" for anything that stores, processes or transmits PHI. Re-check the list at build time and before each new service. Expected set: ECS (Fargate), ECR, RDS (PostgreSQL), S3, KMS, Secrets Manager, ALB, ACM, WAF, CloudWatch (Logs, Alarms), CloudTrail, VPC, SNS, AWS Backup, GuardDuty, Config. If Cognito replaces the self-hosted login (02 ADR-B4 Phase 3), it needs its own check.
- [ ] No PHI in services outside the BAA scope (for example third-party error trackers; the architecture says Sentry only with PII scrubbing and not for the iOS app, 02 section 6).
- [ ] Account guardrails: root account MFA, no root keys, SCP or Config rules blocking public S3, unencrypted EBS/RDS, and non-approved regions.

## 3. Network and transport

- [ ] VPC with private subnets for ECS tasks and RDS; no public IP on RDS or tasks; only the ALB is public.
- [ ] ALB with ACM certificate, TLS 1.2 minimum (prefer a TLS 1.3 policy); HTTP redirected to HTTPS. The app sets no HSTS (`BE/src/app.rs`, `apps/curv_console/next.config.ts`), so add it at the ALB or in a follow-up code change.
- [ ] iOS requires https and a DNS host (`apps/ios_asd_gaze_app/Sources/Models/ResearchStudyConfiguration.swift`): the public API needs a real DNS name.
- [ ] `ASD_TRUST_PROXY_HEADERS=true` in AWS (client IP from the last `X-Forwarded-For` hop) so rate limits and audit IPs are the client's, not the ALB's (`BE/src/auth/mod.rs`). Verify the ALB is the only path to the task, otherwise the header can be forged.
- [ ] Console origin set exactly: `ASD_CONSOLE_ORIGIN=https://<console host>` (`BE/src/config.rs`); `ASD_PUBLIC_BASE_URL` set for webhook download links.
- [ ] Egress control: tasks reach only RDS, S3 (VPC gateway endpoint), KMS, Secrets Manager, CloudWatch, and the internet for webhook and SFTP deliveries through a NAT. The app already blocks private and metadata ranges for webhooks (`BE/src/interop/netguard.rs`), but IMDS must also be v2-only with hop limit 1 (on Fargate use task roles; no instance profile).
- [ ] SFTP deliveries to hospital private addresses need a site VPN or peering; the guard allows RFC 1918 for SFTP only, by design.
- [ ] Console is a separate deployment (Next.js server); decide where it runs (also in scope for the BAA if it handles PHI pages) and put it behind the same ALB/WAF.

## 4. Encryption and keys

| Item | Requirement | Code fact |
|---|---|---|
| RDS | Storage encryption with a customer-managed KMS key; encrypted snapshots; `rds.force_ssl=1`; connection URL with `sslmode=require` or stricter | `DATABASE_URL` only checks the scheme (`BE/src/config.rs`) |
| S3 | Default bucket encryption SSE-KMS with a CMK; bucket policy denies non-TLS and public access; versioning; separate prefixes already per org (`sessions/{org}/...`, `exports/{org}/...`) | The code sets **no** SSE headers (`BE/src/blob/s3.rs`), so bucket default encryption is mandatory |
| Application KEK (`ASD_DATA_KEK`) | Generate and hold in Secrets Manager or KMS; versioned backup in a separate account or region; access only for the task role | Today a base64 32-byte env var (`BE/src/crypto/keyring.rs`). **Losing it loses all clinical text and TOTP seeds** (`rust/.env.example`) |
| KMS-backed KEK (recommended) | Replace the env KEK by KMS: either `GenerateDataKey` once at start and keep it in memory, or KMS Decrypt of a stored wrapped KEK. Needs a code change in `crypto::Kek` (not done) | Owner decision O-9 |
| KEK rotation | A tool that re-wraps `org_data_keys` and re-seals `users.totp_secret_ct` and `integration_destinations.secret_ct` must exist before production | **Not built** (threat model F1) |
| JWT and bundle signing keys | Two different Ed25519 keys in Secrets Manager; bundle key ideally in KMS (02 ADR-B6 says KMS in Phase 3) | Server refuses equal keys (`BE/src/auth/mod.rs`) |
| EBS and ephemeral | Fargate ephemeral storage encrypted; the app writes no PHI to local disk by design (blobs go to S3) | Verify at build time |
| Frames blobs | Already sealed with the device key before upload | `domains/asd_screening/ios/Sources/CurvAsdGazeKit/SessionVaultManager.swift` |

## 5. Identity and access

- [ ] Task roles with least privilege: API role may `PutObject`/`GetObject`/`DeleteObject` only on the session and export prefixes, and use the CMK; worker role the same plus retention and erasure deletes. No `s3:*`, no wildcard resources.
- [ ] Database: migrate as the schema owner, run the API and worker as a member of `asd_app` (for example `asd_api`), **not** the owner. `asd_app` cannot UPDATE or DELETE `audit_log` (`BE/migrations/0002_auth_grants.sql`); `rust/docker-compose.yml` still runs as the owner today.
- [ ] Secrets in Secrets Manager injected as task secrets: `DATABASE_URL`, `ASD_JWT_SIGNING_KEY`, `ASD_BUNDLE_SIGNING_KEY`, `ASD_DATA_KEK`. `rust/.env.example` must not carry real values; `.env` stays git-ignored.
- [ ] Human access: SSO with MFA, break-glass role with alarms, no long-lived IAM user keys. Production database access only through a bastion or SSM session, logged.
- [ ] Production settings: `ASD_ALLOW_SYNTHETIC=false`; `BLOB_MODE=presigned`, `ASD_BLOB_BACKEND=s3` (`BE/src/config.rs`; memory backend must never run in AWS).
- [ ] Vendor staff access to PHI: none by design for `superadmin` (`BE/src/auth/rbac.rs`); infrastructure admins who can read the DB and the KEK can still reach it, so restrict and log that path (O-14).

## 6. Logging, monitoring and audit without PHI

- [ ] Application logs: JSON from `tracing` with the redaction layer on (`BE/src/telemetry.rs`); keep `RUST_LOG` at `info` (debug could add fields). Run `BE/tests/log_redaction.rs` in CI for every release.
- [ ] ALB access logs go to a dedicated encrypted bucket; the path may contain participant tokens (for example `/v1/participants/{token}/summary`) and download tokens in the query string of `/v1/exports/{id}/file?t=`. Decide: disable ALB access logging, or shorten its retention, or accept tokens as pseudonymous. WAF logs likewise. (The app avoids logging raw paths itself, but the ALB does not.)
- [ ] CloudTrail (management and S3 data events for the PHI buckets) to an immutable bucket (Object Lock) in a separate account.
- [ ] Copy `audit_log` to write-once storage (threat model R9); define retention (O-5).
- [ ] CloudWatch log groups encrypted with KMS, retention set explicitly (for example 90 to 400 days; owner choice), access limited.
- [ ] Alarms: 5xx rate, login failure spikes, `auth.refresh_reuse_detected` and `auth.login_totp_failed` audit counts, job failures, RDS storage and CPU, KMS decrypt errors (a symptom of a wrong KEK, which returns `503 clinical-crypto-unavailable`), export and erasure job backlog, GuardDuty findings.
- [ ] The Prometheus `/metrics` endpoint promised in 02 section 4 does not exist in the code (only `/health` and `/health/ready`, `BE/src/routes/health.rs`; `BE/src/routes/metrics.rs` is the per-org console dashboard). If it is added later it must not be public. Expose `/health/ready` to the ALB target group only.

## 7. Backups, retention, deletion

- [ ] RDS automated backups with an owner-chosen window (O-12) and point-in-time recovery; encrypted snapshots; cross-region copy only if O-7 allows.
- [ ] Restore runbook: after any restore, re-run erasure for every `erasure_requests` row newer than the backup (tombstones and requests live in the DB) and re-run the retention sweep. Test it once before go-live.
- [ ] S3 versioning is on, so deletes by the retention and erasure jobs leave noncurrent versions: add a lifecycle rule that expires noncurrent versions soon (days, not months), otherwise "deleted" frames and exports persist (`BE/src/jobs/retention.rs`, `BE/src/jobs/erasure.rs`). Also an abort-incomplete-multipart rule (exports use multipart upload, `BE/src/blob/s3.rs`).
- [ ] Export bucket prefix lifecycle as a second line behind the app's expiry sweep (72 h default, max 168 h, `rust/crates/asd-contract/src/export.rs`): for example expire `exports/` after 8 days.
- [ ] KEK and signing key backups tested (restore into a clean account).
- [ ] AWS Backup for any other stateful store; no PHI in AMIs or container images (CI check).

## 8. Application settings and jobs to verify in AWS

| Check | Reason |
|---|---|
| Run both `asd-backend serve` and `asd-backend worker` (`BE/src/main.rs`); at least one worker must have the blob store | The expiry sweep, nightly retention sweep and erasure scheduling run only in a worker with a blob store (`BE/src/jobs/runner.rs`) |
| Run `asd-backend migrate` with `ASD_DATA_KEK` set, then once `asd-backend admin encrypt-review-notes` | Converts plaintext TOTP seeds and review notes (`BE/src/admin.rs`, `BE/src/auth/totp.rs`) |
| More than one API replica | In-memory rate limiters multiply by replica count; WAF rate rules must be the real control (threat model R4) |
| Clock sync | TOTP and signed links depend on time; use Amazon Time Sync defaults |
| Container hardening | Image scan in ECR, non-root already (`rust/Dockerfile`), read-only root filesystem, drop capabilities, `cargo deny` in CI (02 section 7) |
| WAF | Managed rule sets plus rate-based rules on `/v1/auth/login`, `/v1/devices/register`, `/v1/sessions` (limits in `BE/src/routes/auth.rs`, `BE/src/routes/devices.rs`, `BE/src/routes/sessions.rs`) |

## 9. Documents and evidence to collect before go-live

- [ ] Signed BAA with AWS (Artifact) and the date.
- [ ] BAAs or DPAs with each customer clinic and any subprocessor.
- [ ] This pack reviewed and signed (`dpia-lite.md` section 12).
- [ ] Risk analysis updated for the AWS design (replace the "NOT IMPLEMENTED" rows in `threat-model.md`).
- [ ] Pen test or external review (follow-up F12 in `threat-model.md`).
- [ ] Incident response plan and breach notification procedure (O-11).
- [ ] Workforce access list and training record for anyone with infrastructure access.
- [ ] Evidence export: IaC repository tag, Config conformance report, KMS key policy, bucket policies, WAF rules.

## 10. Exit criteria for "BAA-ready"

All boxes in sections 2 to 7 checked, the open owner decisions O-1, O-7, O-8, O-9, O-12 closed, KEK rotation tooling available, TOTP enrolment shipped and enforced for privileged roles (threat model R2), and the WP-B.7 acceptance test (smoke run of ingest, review, export, erasure, retention against the AWS stack) passing.
