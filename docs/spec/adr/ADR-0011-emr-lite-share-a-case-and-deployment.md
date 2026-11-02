# ADR-0011: Vision-driven scope: EMR-lite record, share-a-case export, HIPAA-safeguard mapping, multi-OS Rust core, one-command deployment

Status: Proposed, 2026-11-02 (owner vision recorded the same day in `docs/product/VISION.md`)
Spec ids: REC-*, SHR-*, SYNC-*, DEP-*, HIP-*, AND-* in
`docs/spec/modules/asd-emr-lite-and-sharing/technical.md`
Packages: phase 7 (tracker `04 §9`): wave 1 WP-B.17, WP-R.7, WP-I.12, WP-B.18, WP-B.20,
WP-B.22, WP-A.12, WP-A.15, WP-A.16a, WP-C.21; wave 2 WP-B.19, WP-I.13, WP-C.18, WP-C.19,
WP-A.14, WP-B.23; wave 3 WP-B.21, WP-C.20, WP-A.16b, WP-E.8

## Context

On 2026-11-02 the owner corrected a drift toward a full clinical-reporting and
hospital-integration product. The mission is a low-friction helping hand for clinics in
low-resource countries, Nepal first, that have no electronic record at all: an easy mobile
app to collect observations and a console to organise them, which together become the
clinic's record system; the ability to share one case with a doctor or researcher in the
US; security designed to HIPAA's technical safeguards and Nepal's Privacy Act 2018;
multi-OS clients over one Rust core; a deployment a non-engineer can run.
`docs/product/VISION.md` is the plain-language statement. This ADR records what that
changes in the architecture that phases 1–6 built.

What exists and was verified at `fe0ed56` (file references in the module spec):

- **Hospital bridge (ADR-0008).** Built: HL7 v2 ORU^R01 encoder (WP-I.4), SFTP/webhook
  delivery with auto-deliver on final sign (WP-I.7), integration settings (WP-I.8).
  Backlog: SMART Backend Services push (WP-I.9), SMART EHR launch (WP-I.10), MLLP
  (WP-I.11), FHIR façade (WP-B.10). PDF/A with Devanagari (WP-I.5) and letters (WP-C.12)
  are still todo, blocked on fonts (ADR-0008 Q3).
- **Clinical record pieces.** Participants (`participants`: token, age bracket, language,
  consent, mode; no create endpoint, rows are upserted at ingest), optional encrypted MRN
  with blind index (MRN-1..6), clinical review with hash-bound signatures (SIGN-*),
  append-only encrypted notes (NOTE-1/2), timeline (CW-3), care events (WP-B.6, no iOS
  caller), console participant workspace (WP-C.14). No encounters/visits, no
  attachments, no follow-ups, no participant search beyond MRN exact match, no name.
- **Participant continuity is broken in clinic mode.** The app mints the wire token as
  `child-<sessionId prefix 8>` unless a research enrolment exists
  (`apps/ios_asd_gaze_app/Sources/Models/GazeSessionRecord.swift:215-219`), and the
  backend upserts `participants(org_id, token)` at ingest. Every clinic session is a new
  participant on the server; a longitudinal record is impossible today.
- **Exports.** One format per export (`asd-contract::export::ExportFormat`), consent
  policy and de-identification in `asd-export` (`policy.rs`, `deid.rs`), watermark,
  `exports`/`export_items` accounting, disclosures endpoint, 5-minute download URLs. FHIR
  R4 session bundle (iOS parity) and clinical bundle (DiagnosticReport, Practitioner,
  Provenance). No single-participant package, no way to hand a package to someone without
  a console account.
- **Offline sync (iOS).** Disk-backed queue with per-blob progress, backoff 1/2/4/8/60 s,
  six attempts then dead-letter, re-register once on 401. Gaps: enqueueing a session does
  not start a flush, app launch does not flush, `isOnline` starts `true` so a device that
  launches online never auto-flushes, no background upload (a blob PUT is one in-memory
  60 s request), the Swift backoff table duplicates `asd-sync::retry`, every `409` is
  treated as "delivered" (including `participant-withdrawn` and
  `idempotency-key-reused`), no fault-injection test. Backend ingest is idempotent (JCS
  body hash, `200 duplicate:true` vs `409`), blob PUT accepts no checksum.
- **Security controls** (survey in the module spec, HIP table below): Argon2id,
  EdDSA JWT 15 min, rotating refresh 30 d, TOTP at login and step-up, mandatory TOTP per
  role (ADR-0010), 33 RBAC actions with a vendor carve-out, envelope encryption of clinical
  text and MRN, append-only `audit_log` (writes audited; most reads are not), HSTS, console
  CSP, redacted logs, erasure and retention jobs. **No TLS termination in the stack, no
  backup or restore, no console idle timeout, no audit export, no org-admin TOTP reset.**
- **Deployment.** `rust/docker-compose.yml` (api, worker, postgres, minio, optional
  Cloudflare tunnel) for dev and demo; the console has no Dockerfile; `aws_infra/` is the
  flood project's Python CDK; WP-B.7 (ECS/RDS) and H-11 (IaC language) are open.
- **Multi-OS.** Rust crates `asd-core`, `asd-contract`, `asd-sync`, `asd-export`,
  `asd-ffi` (UniFFI 0.32, Swift bindings, xcframework), `asd-wasm` (console). Android:
  `uniffi.toml` has a Kotlin package name, the crate has no `cdylib` type, no build script,
  no Gradle module; `scripts/build_android_aar.sh` is the C++ core. No desktop client.

## Decisions

1. **Scope follows the vision: the hospital-push half of ADR-0008 is deferred, the
   record-keeping half becomes core.** Deferred with reason "no hospital systems at the
   target clinics": WP-I.9, WP-I.10, WP-I.11, WP-B.10 (status `deferred`, not deleted).
   Built pieces stay as they are and are not extended: the HL7 encoder, SFTP/webhook
   delivery and integration settings keep passing their tests; `trigger = on_final_sign`
   remains available for the rare clinic that has somewhere to send. Letters (WP-C.12) are
   reduced to the printable clinical summary (NOTE-5); referral letters wait for a clinic
   that asks. Clinical sign-off (SIGN-*) is kept because a record needs an author and a
   time, but it is one clinician's statement about an observation, never a report "for the
   hospital": cosign policy stays `none` by default and the UI words are "sign the note".
2. **The console is the clinic's record (EMR-lite), with a small, explicit data model.**
   A participant is the patient record. New: **encounters** (a visit: kind, date,
   clinician, reason, clinician-written summary (encrypted), status `open|closed`,
   optional `follow_up_due_at`), **attachments** (a photo of a paper form, a referral, a
   PDF: metadata in Postgres, bytes encrypted under the org DEK before they reach the blob
   store), sessions and notes optionally linked to an encounter, a **follow-ups due**
   view, and **participant search** (token prefix, MRN exact, age bracket, last-seen range;
   name only after decision 3). Observations stay what they are (sessions and clip rows);
   care events stay as the parent-side diary. Nothing is pre-filled from the attention
   flag (SIGN-2 extends to encounter summaries). RBAC: `admin` is the front desk (creates
   participants and encounter metadata, never reads clinical text); `clinician` writes
   summaries and notes; `superadmin` keeps the vendor carve-out (RBAC-3). No new role in
   v1 (open question 2). Contract types live in `asd-contract::emr` and are exported into
   `api-v1.json` (CTR-1 pattern).
3. **Identity in the record is an owner decision, designed so either answer is cheap.**
   ADR-0008 D4 ("no names, the EHR beside us holds identity") assumed an EHR that the
   target clinic does not have. The recommended answer to ADR-0008 Q1 is now **yes, opt-in
   per org**: `identity.patient_ref_mode` gains `local_record`, which stores given name,
   family name, sex, birth month and guardian phone encrypted under the org DEK with a
   blind index on the normalised name for exact search, never in de-identified output,
   every decrypt audited, cleared by erasure. Until the owner decides, the contract carries
   the optional `ParticipantIdentity` type and nothing stores it (wave 3, WP-B.21/C.20).
   The MRN mode stays for clinics that keep a paper register with numbers.
4. **Participant continuity: the device captures for a known participant.** Wave 1
   (WP-A.16a): the token is minted once per child profile and reused for every session of
   that profile (`child-<uuid>` stored in the profile; research enrolment still wins).
   Wave 3 (WP-A.16b, after the record API exists): in clinic mode the app scans the
   participant QR the console prints (`asd://participant/<token>?org=<id>`) or picks from
   the device's recent list, so a session recorded on any clinic device lands on the
   record the front desk created. `POST /v1/orgs/{id}/participants` is added (WP-B.19) so a
   record can exist before the first capture; ingest keeps its upsert for parent-mode
   devices.
5. **"Share a case" is one portable, consented, audited package and replaces the
   hospital bridge as the way out.** A new `asd-export::case_package` renders a ZIP:
   `manifest.json` (schema `case-package-v1.json`, exported by `asd-cli schema export`),
   `README.txt` (watermark, mandatory sentences, how to verify hashes), `fhir/bundle.json`
   (the existing clinical bundle encoder, every session of the case), `report/case-summary.pdf`
   (the existing PDF renderer; PDF/A and Devanagari arrive with WP-I.5), `data/sessions.jsonl`
   and `data/clips.csv` (the existing de-identified dataset rows and canonical CSV), and in
   identified mode only `notes/notes.json`, the MRN and attachments. The same `deid.rs`
   rules and one per-package pseudonym salt apply to every file, so a reader can join them.
   The backend adds `ExportFormat::case_package` (one participant, all or selected
   sessions, `purpose ∈ {clinical_care, patient_request}` for identified, any purpose for
   de-identified), reuses the export job, `exports`/`export_items`, disclosures and
   download URLs. The request records **recipient** (free text, encrypted), **purpose**
   and a **consent reference** (which consent, when, by whom); identified sharing refuses
   without one. What counts as valid consent for a cross-border disclosure is **not**
   decided here: it is DPIA O-6 (consent capture) and O-2/O-15 (lawful basis, recipient
   terms); the package only makes the facts mandatory and visible.
6. **Hand-over without an account: a short-lived share link with a passcode.** The
   recipient in the US has no console login. `POST /v1/exports/{id}/share-link` creates a
   link (`/share/{link_id}` in the console, public route) protected by a server-generated
   passcode (≥ 10 characters, Argon2id-hashed, shown once to the sender, sent by another
   channel), `expires_at` ≤ 7 days, `max_downloads` (default 1), per-IP and per-link rate
   limits, every open and download audited with the recipient IP. The link serves the
   export artifact; the artifact's own TTL (EXJ-4) still applies, so the sender sees a
   single expiry. E-mailing files, cloud drives and SMART push are not alternatives we
   build (Alternatives). The ZIP is not itself encrypted in v1 (open question 4).
7. **Offline-first is made true on the device, and proven with a field harness.** The
   queue flushes on enqueue, on launch, on foreground and when a retry time passes while
   online; `isOnline` is unknown until the path monitor reports; blob PUTs become
   background `URLSession` upload tasks from file so iOS resumes them after suspension and
   relaunch; dead-lettered items are retried automatically once per reconnect; the Swift
   backoff copy is deleted in favour of `asd-ffi` `syncBackoffSchedule`/`syncNextRetryDelay`;
   `asd-sync::classify_response` reads the problem slug on `409` so `participant-withdrawn`
   and `idempotency-key-reused` quarantine instead of pretending delivery; the device
   sends `Content-Digest: sha-256=…` (RFC 9530) on blob PUTs and the backend verifies it
   when present. A fault-injecting transport in the iOS tests and a toxiproxy script
   against the compose stack (latency, loss, bandwidth, mid-body cuts) are the acceptance
   tests, with "exactly one row per session, zero dead-letters" as the pass criterion.
   Nothing here changes the ingest contract (ING-*, CON-17), which already guarantees
   idempotent replay.
8. **Deployment is one Docker Compose stack on one server, installed by one command;
   this answers H-11 and retires WP-B.7.** `deploy/clinic-server/` holds
   `docker-compose.yml` (caddy, api, worker, postgres, minio, console, backup timer),
   `install.sh` (asks for the domain and TLS mode, generates every secret, prints the
   key card once, starts the stack, creates the first org and owner), `upgrade.sh`
   (pull, `migrate`, restart, with the version pin in `.env`), `backup.sh` (nightly
   `pg_dump -Fc` + MinIO mirror + erasure ledger, encrypted with `age` to the operator's
   public key, local retention 14 daily + 8 weekly, optional `rclone` off-site),
   `restore.sh` (decrypt, restore, `migrate`, replay erasures through
   `asd-backend admin requeue-erasure`), `restore-drill.sh` (monthly: restore into a
   scratch project on other ports, compare counts with the backup manifest, write a
   PASS/FAIL file). TLS modes: `letsencrypt` (public DNS name, default), `internal`
   (Caddy's own CA, profile installed on the iPads; documented), `tunnel` (the existing
   cloudflared service). Postgres and MinIO are never published on host ports; the API
   and worker connect as `asd_app`, not the owner role (threat-model F10); the console
   image comes from a new `apps/curv_console/Dockerfile` (`next build` standalone).
   `rust/docker-compose.yml` stays the dev/demo stack. `aws_infra/` stays the flood
   project's CDK and gets no `asd/` app; if a clinic or a US host ever needs the stack on
   AWS, the same compose runs on one EC2 instance (region = O-7/O-8), and that is a
   runbook, not code. The owner's AWS BAA checklist (`docs/security/aws-baa-checklist.md`)
   remains the reference if that day comes.
9. **Security is designed to HIPAA's technical safeguards (45 CFR 164.312) and the
   Privacy Act 2018 (Nepal); the administrative and legal parts are the owner's.** HIPAA
   legally binds US covered entities and their business associates; a clinic in Nepal is
   not one, and a US doctor who receives a case may be one for the copy they hold. We
   therefore implement the technical safeguards as engineering MUSTs (HIP-*), make the
   package a recipient can hold compliantly (integrity hashes, disclosure record,
   de-identified by default), and list what is outside code: risk analysis, policies and
   procedures, workforce training, sanction policy, business-associate and
   data-processing agreements (O-15), breach-notification process (O-11), contingency
   plan ownership (O-12). WP-E.8 writes that pack with the owner. The mapping:

   | Safeguard (164.312 unless noted) | Existing control (verified) | Gap | Package |
   | --- | --- | --- | --- |
   | (a)(1) Access control: unique user id, role-based | `users`, 7 roles, 33 actions, org scoping 404, vendor carve-out (RBAC-3) | front-desk role reuse of `admin`; new record actions | B.19 (actions), owner Q2 |
   | (a)(2)(ii) Emergency access | none; `asd-backend admin` on the server is the only recovery | procedure, not code (O-14: no vendor break-glass) | E.8 |
   | (a)(2)(iii) Automatic logoff | none in the console (`lib/auth/cookies.ts` only sets `max-age`) | idle 30 min / absolute 12 h (O-10 second half, constants) | C.21 |
   | (a)(2)(iv) Encryption at rest | AES-256-GCM envelope for clinical text, MRN, TOTP seeds, integration secrets; `ASD_DATA_KEK` + rotation | session summaries, care events, blobs rely on disk/volume encryption; attachments | B.19 (attachments under DEK), B.18 (install checks full-disk encryption and refuses to continue without `--allow-unencrypted-disk`), O-9 KEK custody |
   | (b) Audit controls | append-only `audit_log` (triggers, revoked grants), ~70 write actions, `GET /orgs/{id}/audit` | reads of session detail, participant, timeline, clinical review, notes, disclosures and the served export file are **not** audited; no off-box copy; no export | B.20 (read audits), B.18 (encrypted backups hold the log), B.23 (audit CSV), O-5 retention |
   | (c)(1) Integrity | JCS content hash + signatures, immutability triggers, `sessions.request_hash`, signed config bundles, `exports.sha256` | blob uploads unverified; dataset manifest without hashes | A.15 + B.20 (`Content-Digest`), I.12 (every package file hashed in the manifest) |
   | (d) Person or entity authentication | Argon2id, EdDSA JWT 15 min, refresh rotation with reuse detection, TOTP at login and step-up, mandatory TOTP per role, hashed device tokens and enrol codes, rate limits | no org-admin TOTP reset (staff who lose a phone need an engineer); no lockout (rate limits only, accepted) | B.17 |
   | (e) Transmission security | HSTS on API and console, iOS https+DNS only, rustls outbound, pinned SFTP host keys | the stack itself serves plain HTTP (TLS was "a proxy later"); API published on all interfaces | B.18 (Caddy TLS, private network, DB/MinIO unpublished) |
   | 164.308(a)(7) Contingency: backup, DR, testing | none | everything | B.18, B.22 (erasure replay), O-12 |
   | 164.310(d)(2) Disposal, re-use | erasure job with tombstone and hold, retention sweep, export expiry, housekeeping | restores can resurrect erased data | B.22 ledger + restore replay, O-12, O-1 |
   | 164.528 Accounting of disclosures | `exports`, `export_items` (survive erasure), disclosures endpoint, watermark | disclosures read not audited; case package and share links must land in the same tables | B.20, I.13 |
   | Minimum necessary / de-identification | `deid.rs`, consent policy, research = de-identified only | cross-border rules not decided | I.12/I.13 make facts mandatory; O-2, O-6, O-15 decide |
   | Administrative safeguards (164.308), BAAs (164.314), policies (164.316) | `docs/security/*` (threat model, DPIA-lite, retention policy, AWS BAA checklist) | not code | E.8 + owner |

10. **Multi-OS architecture: one Rust core, four thin shells, in this order.**
    - **Rust crates (exist):** `asd-core` (maths), `asd-contract` (contracts, schema,
      signing; gains `emr`), `asd-sync` (protocol), `asd-export` (formats, de-id; gains
      `case_package`), `asd-ffi` (UniFFI), `asd-wasm`, `asd-backend`, `asd-cli`. No
      platform re-implements any of it (`CLAUDE.md`).
    - **iOS/visionOS (exists):** Swift over `AsdCore.xcframework` (ADR-0006 D7).
    - **Android (WP-A.12, wave 1):** the same UniFFI definitions produce Kotlin bindings;
      `asd-ffi` gains `cdylib`, `scripts/build_asd_android.sh` (cargo-ndk, arm64-v8a /
      armeabi-v7a / x86_64), a Gradle library `apps/android_asd_core/` (AAR with JNA)
      whose unit test runs the golden scoring vectors through the bindings (FFI-4 for
      Android). This is independent of the capture-device decision: it is the floor
      either answer stands on.
    - **Android capture feasibility (WP-A.14, wave 2, owner-facing):** a spike, not a
      product package: ARCore Augmented Faces and MediaPipe Face Landmarker on two cheap
      phones, raw tracking logs in the Rust raw-log format, replayed by
      `asd-cli gaze replay` and validated with the same degrees-of-accuracy pipeline
      (ADR-0007). Output is a one-page number sheet for the owner (accuracy in degrees,
      valid-frame ratio, frame rate, cost per device). No promise about Android gaze is
      made before that sheet exists.
    - **Web (exists):** Next.js console over `asd-wasm` for validation and sandbox
      re-score; it renders nothing that is a format or a rule (UI-2).
    - **Desktop: no desktop application now; the console is the desktop UI.** It is
      opened in a browser and may be "installed" as a PWA (manifest + icons, a one-line
      package when wanted). When a desktop binary is genuinely needed (kiosk mode, OS
      integration, a clinic that bans browsers), the chosen shell is **Tauri 2** loading
      the clinic server's console URL in remote mode, because it is Rust, small, and reuses
      the console unchanged. Tauri cannot bundle the console offline: since WP-C.16 every
      route is dynamic (per-request CSP nonce, `connection()`), so a static export does
      not exist and bundling would need a Node sidecar. That is recorded as the condition
      for revisiting (Alternatives).
11. **WP-B.17 (org-admin TOTP reset) moves to wave 1.** Staff lose phones; a clinic has
    no engineer with server access. `POST /v1/orgs/{id}/users/{uid}/totp/reset` for
    `owner` and `admin` (an `admin` cannot reset an `owner`), step-up re-authentication by
    the caller (SIGN-5 rules), clears both seed columns, revokes the target's refresh
    token families, audits `user.totp_reset_by_admin` with both ids and nothing else. The
    target is then `required` again under ADR-0010 D6 and can only enrol.
12. **Migration numbers:** 0024 WP-B.19 (encounters, attachments, encounter links,
    follow-up), 0025 WP-I.13 (share links, case-share fields on `exports`), 0026 WP-B.21
    (identity, after decision 3). Wave 1 adds no migration. The tracker rule becomes
    "≥0027" once wave 3 lands; packages that add a `sqlx::query!` regenerate `rust/.sqlx/`.

## Alternatives rejected

- **Keep building the hospital bridge (SMART push, MLLP, façade) "because it is
  designed".** No target clinic has a receiving system; each piece is a month of work and
  a permanent security surface. Deferred, not deleted: the ADR-0008 design still applies
  if a hospital appears.
- **A separate "EMR" product or a fork of an open-source EMR (OpenMRS, Bahmni) next to the
  console.** Two logins, two data models, a Java stack to operate, and the gaze
  observations would still need an integration. The clinic needs ten tables, not an EMR.
- **Store identity in clear "because the clinic has nothing else".** Rejected; identity,
  if the owner allows it, goes under the same envelope encryption as notes with a blind
  index, exactly like the MRN.
- **Share by e-mail attachment or by a cloud-drive link generated by the console.** PHI in
  mail servers and third-party drives outside the disclosure record; no expiry, no
  download audit. The share link keeps the artifact on the clinic server and records
  every access.
- **Encrypt the case ZIP with the passcode (or `age`) in v1.** It doubles as a second
  factor for the file at rest on the recipient's laptop, but PDF/FHIR tools cannot open
  it directly and support calls would follow. Left as open question 4; the link and TLS
  protect transit, the recipient's obligations cover rest.
- **FHIR `document` bundle with a `Composition` as the package.** A richer, more standard
  container, but readers would still want the PDF and the CSV; the ZIP with a manifest is
  what a researcher opens first. The FHIR bundle inside keeps ADR-0003/FHX-* shapes.
- **Background modes or BGProcessingTask for sync.** A background `URLSession` upload
  task from file gives OS-managed resumption without a background mode entitlement and
  without App Review questions; BGTaskScheduler is opportunistic and would not help the
  mid-upload drop.
- **Kubernetes (k3s), Nomad, or a cloud-managed stack (ECS/RDS, WP-B.7) for the clinic
  server.** A nurse-run clinic has one box and no SRE; Compose plus shell plus Caddy is
  what one person can restore at 2 a.m. The cloud variant is a runbook on the same compose.
- **Terraform/CDK for a single server.** Nothing to orchestrate; it would add a toolchain
  to install before the first command.
- **Electron desktop.** Node runtime, 150 MB, not Rust, and the console still needs the
  server. Rejected.
- **Native Rust desktop UI (egui, iced, Slint, Dioxus).** A second UI implementation of
  every console screen. Rejected; the console is the UI.
- **Tauri with a Node sidecar to bundle the console offline.** Possible, but it ships a
  Node runtime inside a "Rust" app and the console is useless without the server anyway.
  Revisit only if a clinic needs the console on a laptop that cannot reach the server.
- **Promising Android capture now, or ruling it out now.** Front-camera gaze on cheap
  phones without a depth sensor is unproven for toddlers; the spike decides with numbers.
- **A new `frontdesk` role in v1.** Touches the RBAC matrix, the TOTP policy constants,
  the console nav and every mock. `admin` already matches a receptionist's needs (no
  clinical content). Open question 2 if the owner wants it.

## Consequences

- Tracker rows WP-I.9, I.10, I.11, B.10 become `deferred`; WP-B.7 becomes `superseded by
  WP-B.18`; WP-B.17 moves to phase 7 wave 1; WP-C.12 is re-scoped to the clinical summary
  endpoint.
- `aws_infra/` is explicitly flood-only; `docs/roadmap_2026q4/02 §2` ADR-B7's "Phase 3:
  AWS" is superseded by this ADR's decision 8 (noted in the module spec, the file is not
  rewritten).
- ADR-0008 D4 ("no names, ever, v1") is reopened as decision 3 with a recommendation; the
  ADR text stays as history.
- New public surface: the share-link routes are the first unauthenticated routes that
  serve participant data; they are rate-limited, passcode-gated, short-lived and audited,
  and the threat model (`docs/security/threat-model.md §4.4`) must gain a row for them
  (WP-E.8).
- `asd-ffi` gains a `cdylib` crate type (Android needs a shared library); the Apple build
  keeps using the static library. `cargo deny` sees no new Rust dependencies for Android;
  JNA is a Gradle dependency (Apache-2.0).
- `apps/curv_console/next.config.ts` gains `output: "standalone"` for the Docker image;
  the CSP/HSTS logic there is untouched.
- Every read of clinical content writes an audit row (B.20); `audit_log` grows faster,
  which makes O-5 (retention, off-box copy) more pressing. Backups (B.18) are the off-box
  copy until then.
- Wave 1 adds no migration and no `api-v1.json` change except the new `emr` `$defs`
  (R.7) and B.17's DTO, both regenerated once by the main session.
- The field harness becomes a gate for any later change to the sync client.

## Open questions for the owner

1. **Capture device** (recorded, not decided): clinic-owned iPad with TrueDepth (works
   today, ~USD 350–600 per device) versus an Android trial on cheap phones. Recommendation:
   run the pilot on iPads now, run WP-A.14 in parallel, decide on its number sheet. Nothing
   in phase 7 waits for this.
2. **Front-desk role.** Recommendation: reuse `admin` in v1; add `frontdesk` only if a
   clinic objects to the front desk being able to enrol devices and change settings.
3. **Identity in the record** (ADR-0008 Q1 reopened). Recommendation: allow
   `identity.patient_ref_mode = local_record` (decision 3) for clinics without any other
   register; keep `none` the default. Decides whether wave 3 runs.
4. **Encrypt the case ZIP at rest** with the passcode. Recommendation: no in v1.
5. **Share-link limits.** Recommendation: expiry 72 h (max 7 days), 1 download (max 5),
   passcode 12 characters, 10 opens per link, 30 per IP per hour.
6. **Console session policy** (O-10 second half). Recommendation: idle 30 minutes,
   absolute 12 hours, constants in WP-C.21.
7. **TLS mode for the first pilot**: `letsencrypt` with a public DNS name (the server must
   be reachable on 80/443 or use DNS-01), `internal` CA with an iPad profile, or
   `tunnel`. Recommendation: `letsencrypt` if the clinic has a static address or a cheap
   VPS; otherwise `tunnel`.
8. **Backup retention and off-site target** (O-12, O-7): 14 daily + 8 weekly on the box;
   off-site encrypted copies to a bucket in a region the owner chooses.
9. **Cross-border consent text and evidence** (O-6, O-2, O-15): the share request makes a
   consent reference mandatory; what the parent signs and where it is stored is the
   owner's.
10. **PWA install** of the console (icons, manifest): wanted for the pilot or not.
