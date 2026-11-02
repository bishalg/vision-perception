# Risk analysis and risk register

> **DRAFT. Requires legal review before use.** WP-E.8, 2026-11-02. Written for a small clinic with no IT staff. It does not claim that the product or the clinic is HIPAA compliant or certified. Items marked [OWNER DECISION: O-x] are recommended defaults awaiting the owner.

Maps to: 164.308(a)(1)(ii)(A)-(B), (a)(8). Derived from `docs/security/threat-model.md` (written 2026-11-01; several of its "gaps" have since been closed by phase 5-7 work, see the Status column) and ADR-0011. Re-run yearly and after any new data class, recipient, region or major version [OWNER DECISION: O-1..O-15 still shape several rows].

## How to use
Score Likelihood (L) and Impact (I) as Low / Medium / High; priority is the worse of the two. The PSO and owner review the register each year, tick actions, and add new risks after incidents. Sign at the bottom.

## Register
| ID | Risk (plain words) | Threat-model ref | L | I | Controls in place (as of this draft) | Action, owner, due |
| --- | --- | --- | --- | --- | --- | --- |
| RA-1 | The data key (key card) is lost or copied; one secret opens all clinical text | R1, 4.3 | Med | High | Rotation tool `asd-backend admin rotate-kek`; key card printed once; drill checks the key (`restore-drill.sh`) | Two sealed copies; KEK custody [OWNER DECISION: O-9]. Owner |
| RA-2 | Staff accounts without a second factor | R2 | Med | High | Mandatory TOTP for owner/admin/clinician/superadmin, admin reset (TRS-1) | Confirm integration and roles [OWNER DECISION: O-10]. Eng/PSO |
| RA-3 | A family's erasure request cannot fully delete signed clinical records | R3 | High | Med | Hold flow documented | [OWNER DECISION: O-1]. Owner, Legal |
| RA-4 | Login and device-enrol guessing; limiter is per process | R4 | Low | Med | Rate limits; Caddy in front; TOTP | Accept for single-server clinic; revisit if scaled. Eng |
| RA-5 | Core session data is clear in Postgres; protection relies on the server | R5 | Med | Med | Full-disk encryption checked by `install.sh`; DB not published; app-level encryption for clinical text | Keep disk encryption mandatory; column encryption decision. Owner |
| RA-6 | A leaked device token posts bad data or files erasures org-wide | R6 | Low | Med | Revocable tokens, hashed at rest | Per-device scoping, token expiry. Eng |
| RA-7 | Console attacks: cross-site scripting, idle sessions | R7 | Low | High | CSP (WP-C.16), idle logoff HIP-2, httpOnly cookies | Confirm HIP-2 integrated. Eng |
| RA-8 | De-identified data re-identified in a small clinic | R8, 4.4 | Med | Med | Pseudonym salt per export, month-level dates | Minimum cell size rule; no identified data for research. Eng |
| RA-9 | Audit log tampered or lost with the server | R9 | Low | Med | Append-only triggers, run as `asd_app`, copy inside every backup | Audit retention [OWNER DECISION: O-5]. Owner |
| RA-10 | Backups bring back erased data, or fail silently | R10 | Med | Med | `backup.sh`, erasure ledger replay, monthly `restore-drill.sh`, `status.sh` warnings | Document period erased data may persist [OWNER DECISION: O-12]. PSO |
| RA-11 | Server disk stolen or lost | 4.6 | Low | High | Disk encryption mandatory; locked room (`device-and-media.md`) | Annual physical check. PSO |
| RA-12 | Tablet lost or stolen | 4.1 | Med | Med | Encrypted vault, Keychain, passcode policy, remote disable | Register and training. PSO |
| RA-13 | Insider reads a family's record out of curiosity or sells it | 4.2 | Med | High | RBAC, read audits (HIP-1), sanctions, training | Weekly audit glance (`access-control.md`). PSO |
| RA-14 | Wrong server region or provider without terms | O-7/O-8 | Low | High | Cloud only after owner decision (README of `deploy/clinic-server`) | [OWNER DECISION: O-7]. Owner |
| RA-15 | Staff lose their only phone and cannot enter | R2, TRS | High | Low | Org-admin reset, `reset-totp` operator command, two owners | Maintain two owners (`emergency-access.md`). PSO |
| RA-16 | Plaintext secrets or logs leak PHI | M16 | Low | Med | Log redaction tests, plaintext columns dropped (migration 0022) | Add test with every new field. Eng |

### New: share-a-case links (ADR-0011 D6, SHR-8/9, HIP-8/9)
These controls are **specified** (WP-I.13); the rows stay open until the package ships and is tested.

| ID | Risk | L | I | Controls specified | Action, owner |
| --- | --- | --- | --- | --- | --- |
| RA-20 | Link or passcode sent over the same channel and both intercepted | Med | High | Passcode sent separately by the sender; link life 72 h (max 7 days); one download (max 5); 12-char passcode | Training: never put both in one message. PSO |
| RA-21 | Passcode guessing against a public route | Low | High | Argon2id passcode, 10 opens/link/h, 30/IP/h, uniform 404, audit with IP | Eng |
| RA-22 | Link forwarded to someone who is not the intended doctor | Med | High | Expiry, download limit, recipient-IP audit, revoke button, written agreement | Revoke on suspicion. PSO |
| RA-23 | Case sent without valid consent | Med | High | Share request refuses without a consent reference (`case-consent-missing`); signed form attached to the record | Checklist in `templates/cross-border-sharing-consent.md` [OWNER DECISION: O-6]. PSO |
| RA-24 | Identified data to a US recipient with no agreement | Med | High | Identified mode only for `clinical_care` or `patient_request`; template agreement | [OWNER DECISION: O-15]. Owner |
| RA-25 | Recipient keeps the file forever, loses it, or re-shares | High | High | Watermark, manifest hashes, de-identified by default; contract terms for return/delete and breach notice | Agreement and follow-up reminder. Owner |
| RA-26 | Link page cached or indexed by browsers/search engines | Low | Med | `Cache-Control: no-store`, `X-Robots-Tag: noindex`, CSP (HIP-9) | Eng |
| RA-27 | Re-identification of a de-identified case by a researcher | Med | Med | One salt per package, age bracket, month-level dates; small cohort | Minimum cell size; agreement bans re-identification. Eng/Owner |
| RA-28 | Erased child's share link still works | Low | High | Erasure revokes share links (SHR-11) | Test at release. Eng |
| RA-29 | Cross-border transfer without a lawful basis under Nepali law | Med | High | Consent, agreement, audit | Nepali counsel [OWNER DECISION: O-2, O-15]. Legal |

## Sign-off
| Date | Reviewed by (owner) | PSO | Next review due | Notes |
| --- | --- | --- | --- | --- |
| | | | | |
