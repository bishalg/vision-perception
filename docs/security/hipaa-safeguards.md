# HIPAA Security Rule and Nepal Privacy Act mapping

> **DRAFT. Requires legal review before use.** WP-E.8, 2026-11-02. This is an engineering
> and administrative mapping, not legal advice. Nothing here states or implies that the
> product is "HIPAA compliant", certified or approved by any regulator. HIPAA binds US
> covered entities and business associates; a clinic in Nepal is generally not one
> (ADR-0011 decision 9). The mapping shows which of our controls are designed to the same
> standard, so that a US recipient and a Nepali regulator can both see what exists.

Owner decisions are marked **[OWNER DECISION: O-x]** (list in `docs/security/dpia-lite.md`
section 11). Paths: **BE/** = `rust/crates/asd-backend/`, **CON/** = `apps/curv_console/`,
**DEP/** = `deploy/clinic-server/`. Spec ids (HIP-*, DPL-*, SHR-*) are in
`docs/spec/modules/asd-emr-lite-and-sharing/technical.md`.

**Status words.** `built` = in the code or scripts and covered by tests as far as we know;
`specified` = in the spec, package not finished; `policy` = a document in this pack, needs
the clinic to adopt and follow it; `open` = nothing yet or an owner decision is missing.
"Built" statuses that depend on uncommitted work in the tree must be re-confirmed when that
work is integrated (the rows say so).

**Owner column.** `Eng` = engineering (this repository). `PSO` = the clinic's Privacy and
Security Officer, a named person at each clinic (see `policies/access-control.md`).
`Owner` = the product owner/operator. `Legal` = needs a lawyer.

## 1. Administrative safeguards, 45 CFR 164.308

| Standard (implementation spec) | R/A | Our control | Status | Owner |
| --- | --- | --- | --- | --- |
| (a)(1)(i) Security management process | R | `policies/risk-analysis.md`, `docs/security/threat-model.md`, `dpia-lite.md` | policy (draft) | Owner, PSO |
| (a)(1)(ii)(A) Risk analysis | R | `policies/risk-analysis.md` (register from the threat model, including share links) | policy (draft); annual review needed | Owner |
| (a)(1)(ii)(B) Risk management | R | Register actions with owners; engineering backlog F1..F12 in `threat-model.md` section 6 | policy (draft) | Owner |
| (a)(1)(ii)(C) Sanction policy | R | `policies/workforce-training.md` section "Sanctions" | policy (draft) | PSO, Legal |
| (a)(1)(ii)(D) Information system activity review | R | Audit log (BE/src/audit.rs, reads: HIP-1, `rust/README.md` "What is audited"); `audit_csv` export (HIP-3); monthly review checklist in `policies/access-control.md` | built for writes; reads and CSV export: confirm on integration of phase 7 work | Eng, PSO |
| (a)(2) Assigned security responsibility | R | PSO named per clinic; the owner for the platform ([OWNER DECISION: O-11], default: clinic owner is PSO until another person is named) | policy (draft) | Owner |
| (a)(3)(i) Workforce security; (ii)(A) authorization/supervision; (B) clearance; (C) termination | R/A | Roles and RBAC (BE/src/auth/rbac.rs); disable user; `policies/access-control.md` joiner/mover/leaver checklist | built (RBAC); policy (draft) | PSO |
| (a)(4)(i) Information access management; (ii)(B) access authorization; (C) access establishment | R/A | RBAC matrix, org scoping returns 404, vendor `superadmin` denied clinical content (M7 in threat model); front desk uses `admin` in v1 (ADR-0011 Q2) | built | Eng, PSO |
| (a)(5)(i) Security awareness and training | R | `policies/workforce-training.md` | policy (draft) | PSO |
| (a)(5)(ii)(B) Protection from malicious software | A | Server runs Docker images only; OS updates in `policies/device-and-media.md`; no email or browsing on the server | policy (draft) | PSO |
| (a)(5)(ii)(C) Log-in monitoring | A | `auth.*` audit rows, refresh-reuse revocation, rate limits (threat model M1..M4); weekly check in `policies/access-control.md` | built; review is policy | PSO |
| (a)(5)(ii)(D) Password management | A | Argon2id, mandatory TOTP for owner/admin/clinician/superadmin after 14 days (`asd-contract::auth_policy`, rust/README.md "Mandatory two-step sign-in"), org-admin TOTP reset (TRS-1) | built; confirm TOTP work is integrated [OWNER DECISION: O-10 to confirm roles] | Eng, PSO |
| (a)(6)(i) Security incident procedures; (ii) response and reporting | R | `policies/incident-response-and-breach.md`; audit trail; `DEP/status.sh` | policy (draft) | PSO |
| (a)(7)(i) Contingency plan; (ii)(A) data backup | R | `DEP/backup.sh`, `systemd/asd-backup.timer`, key card; `policies/contingency-and-backup.md` | built (WP-B.18); confirm on integration | Eng, PSO |
| (a)(7)(ii)(B) Disaster recovery plan | R | `DEP/restore.sh`, erasure ledger replay (`asd-backend admin requeue-erasure`, WP-B.22) | built; confirm on integration | Eng, PSO |
| (a)(7)(ii)(C) Emergency mode operation plan | R | Paper fallback in `policies/contingency-and-backup.md`; the app captures offline (ADR-0011 decision 7) | policy (draft) | PSO |
| (a)(7)(ii)(D) Testing and revision | A | `DEP/restore-drill.sh`, `systemd/asd-drill.timer` (monthly), `status.sh` warns if stale | built; confirm on integration | PSO |
| (a)(7)(ii)(E) Applications and data criticality analysis | A | Asset table in `threat-model.md` section 1; recovery order in `policies/contingency-and-backup.md` | policy (draft) | Owner |
| (a)(8) Evaluation | R | Re-run the risk analysis yearly and after any new data class, recipient or region (`dpia-lite.md` section 12); external review before a clinical pilot (threat model F12) | open | Owner |
| (b)(1) Business associate contracts; (b)(3) written contract | R | `templates/baa-dpa-us-recipient.md`; AWS agreement per `aws-baa-checklist.md` only if hosted on AWS | policy (draft) | Owner, Legal [OWNER DECISION: O-15] |

## 2. Physical safeguards, 45 CFR 164.310

| Standard | R/A | Our control | Status | Owner |
| --- | --- | --- | --- | --- |
| (a)(1) Facility access controls; (a)(2)(i)-(iv) contingency operations, facility security plan, access control and validation, maintenance records | A | Server in a locked room; `policies/device-and-media.md` sections "Server" and "Visitors" | policy (draft) | PSO |
| (b) Workstation use | R | `policies/device-and-media.md` "Workstations"; console auto-logoff (HIP-2, `CON/src/lib/auth/session-policy.ts`) | policy; HIP-2 confirm on integration | PSO |
| (c) Workstation security | R | Same; clinic-owned tablets with passcode and Find My; iOS vault, Keychain, file protection (M19) | built (app); policy (devices) | PSO |
| (d)(1) Device and media controls; (d)(2)(i) Disposal | R | `policies/device-and-media.md`; erasure job (BE/src/jobs/erasure.rs); crypto-shredding procedure (`retention-policy.md` 1.3) | built (erasure); policy (disposal) | PSO |
| (d)(2)(ii) Media re-use | R | Same; full-disk encryption refused-if-absent by `DEP/install.sh` | built (check) | Eng, PSO |
| (d)(2)(iii) Accountability (movement of hardware) | A | Device register in `policies/device-and-media.md` (a table the clinic keeps) | policy (draft) | PSO |
| (d)(2)(iv) Data backup and storage before moving equipment | A | Run `DEP/backup.sh` first; see `policies/device-and-media.md` | policy (draft) | PSO |

## 3. Technical safeguards, 45 CFR 164.312

| Standard | R/A | Our control | Status | Owner |
| --- | --- | --- | --- | --- |
| (a)(1) Access control; (a)(2)(i) unique user id | R | `users` table, one login per person, roles (BE/src/auth/rbac.rs, tests `rbac_matrix.rs`, `rbac_clinical.rs`) | built | Eng |
| (a)(2)(ii) Emergency access procedure | R | `policies/emergency-access.md`: no vendor break-glass (ADR-0008 D2, [OWNER DECISION: O-14] recommended default: none in v1); recovery via a second owner account, `reset-totp`, key card | policy (draft) | PSO |
| (a)(2)(iii) Automatic logoff | A | Console idle 30 min, absolute 12 h (HIP-2, `CON/src/lib/auth/session-policy.ts`) [OWNER DECISION: O-10 second half]; 15-minute access token (M2) | specified/built in tree; confirm on integration | Eng |
| (a)(2)(iv) Encryption and decryption | A | AES-256-GCM envelope for clinical text, MRN, TOTP seeds, integration secrets (BE/src/crypto/); attachments under DEK (HIP-5, REC-6/7); disk encryption checked by `DEP/install.sh`; KEK custody [OWNER DECISION: O-9] | built; attachments specified | Eng |
| (b) Audit controls | R | Append-only `audit_log` with write-refusing triggers (BE/migrations/0002_auth_grants.sql); read audits HIP-1; audit CSV HIP-3; backups carry the log off the box; retention [OWNER DECISION: O-5] | built for writes; read audits and CSV: confirm on integration | Eng |
| (c)(1) Integrity; (c)(2) authenticate ePHI | A | JCS hash and signatures on signed records, immutability triggers, `exports.sha256`, `Content-Digest` on blob PUTs (HIP-4, SYNC-6), per-file hashes in the case manifest (SHR-2) | built; case manifest specified | Eng |
| (d) Person or entity authentication | R | Argon2id, EdDSA JWT 15 min, refresh rotation with reuse detection, TOTP and step-up (BE/src/auth/), hashed device tokens | built | Eng |
| (e)(1) Transmission security; (e)(2)(i) integrity controls; (ii) encryption | A | Caddy TLS in the clinic stack, only 80/443 published (DPL-3, DEP/Caddyfile, HIP-6); HSTS in API and console; iOS https-only; share-link routes `no-store` (HIP-9) | built (stack); share routes specified | Eng |
| Accounting of disclosures, 164.528 (privacy rule, listed for completeness) | R | `exports`, `export_items` kept after erasure, `GET /v1/participants/{id}/disclosures`; share-link audit with recipient IP (SHR-9) | built; share links specified | Eng |

## 4. Organizational requirements and documentation, 164.314 and 164.316

| Standard | R/A | Our control | Status | Owner |
| --- | --- | --- | --- | --- |
| 164.314(a) Business associate contracts | R | `templates/baa-dpa-us-recipient.md` (recipient abroad); the operator-to-clinic agreement when one operator serves several clinics [OWNER DECISION: O-2 roles] | policy (draft) | Owner, Legal |
| 164.314(b) Group health plans | n/a | Not applicable: no plan sponsor | n/a | n/a |
| 164.316(a) Policies and procedures | R | This pack, `docs/security/policies/` | policy (draft) | PSO |
| 164.316(b)(1) Documentation; (b)(2)(i) time limit | R | Keep policies, risk analyses, training logs, drill results, incident records and signed BAAs for **6 years** from creation or last effective date (HIPAA). Drill results: `/var/lib/asd/drills/*.PASS|.FAIL` | policy (draft); retention of clinical data is separate (`retention-policy.md`, O-3) | PSO |
| 164.316(b)(2)(ii)-(iii) Availability; updates | R | This pack is in git (history is the version trail); review yearly and after incidents | policy (draft) | Owner |

## 5. Breach notification, 164.400-414 (context)

A US recipient that holds an identified case copy may have notification duties to US
individuals and HHS (60-day outer limit). Our side: contractual notice both ways within
the time in `templates/baa-dpa-us-recipient.md`, and our own process in
`policies/incident-response-and-breach.md` [OWNER DECISION: O-11].

## 6. Nepal: Privacy Act 2018 (Act Relating to Privacy, 2075)

> **Legal review required by a Nepali lawyer.** This table lists obligations as we
> understand the Act's subjects. It deliberately gives no section numbers or penalty
> amounts: confirm each against the current official text and any rules or directives
> issued under it, including health-sector rules, and check whether a separate data
> protection law has since been enacted. We do not claim certainty about cross-border
> transfer rules; the Act may not prescribe a regime, so we apply consent and contract
> as the safe default.

| Obligation (as understood) | Our control | Status | Owner |
| --- | --- | --- | --- |
| Privacy of the person, health information and personal data is protected; personal information collected only for a lawful, stated purpose | Purpose-limited collection; `dpia-lite.md` sections 3 and 4; no raw video, pseudonymous token on the wire | built (design); policy | Owner [OWNER DECISION: O-2 lawful basis, default: clinic is controller for care; guardian consent for sharing and research] |
| Consent of the person (for a child, the guardian) for collection and use, and for disclosure to others | In-app consent gate before capture (M19); `consent_version`, `consent_at`; written cross-border consent `templates/cross-border-sharing-consent.md`; the share request refuses without a consent reference (ADR-0011 decision 5) | built (app gate); consent form draft | Owner [OWNER DECISION: O-6] |
| Use only for the purpose collected; no disclosure without consent or legal authority | Purpose field on case packages (`clinical_care`, `patient_request`); de-identified by default; research needs de-identified data and per-session consent (M12) | built | Eng |
| Protect personal information against loss, misuse, unauthorised access | Section 3 technical controls; policies in this pack | built plus policy | Eng, PSO |
| Access to and correction of one's own information | Participant record in the console; clinician amends under sign-off rules; request handling in `policies/access-control.md` "Requests from families" | built (record); policy | PSO |
| Deletion or withdrawal | Erasure job and withdrawal (BE/src/jobs/erasure.rs, `retention-policy.md` 1.2); clinical-record hold open [OWNER DECISION: O-1]; backups keep erased data until rotated out, replay on restore [OWNER DECISION: O-12] | built; hold open | Owner, Legal |
| Sensitive information (health) needs more protection and limited sharing | Encryption, RBAC, read audits, disclosure accounting, share links with passcode, expiry, limits | built / specified | Eng |
| Transfer of personal information abroad | Deliberate, consented, audited act only (VISION.md); agreement with recipient; `cross-border-sharing-consent.md`; legal basis confirmed by Nepali counsel [OWNER DECISION: O-2, O-15] | policy (draft) | Owner, Legal |
| Notify authority or individuals of a breach (confirm whether the Act requires it and when) | `policies/incident-response-and-breach.md` assumes notification to guardians and the relevant authority as soon as practicable [OWNER DECISION: O-11] | policy (draft) | PSO, Legal |
| Registration or approval requirements for holders of sensitive data (confirm) | None built | open | Legal |
| Staff confidentiality duty | `policies/workforce-training.md` confidentiality pledge | policy (draft) | PSO |
| Records of processing | `docs/security/data-flow.md`, `dpia-lite.md`, audit and disclosure records | built (docs) | Owner |

## 7. What this mapping does not cover

No external audit, penetration test or certification exists. Pen-test or external review
before a clinical pilot is open (threat model F12). Items in the threat model that are
marked NOT IMPLEMENTED stay as written there; this file does not override it.
