# Product vision: a helping hand for clinics that have no record system

Owner statement recorded 2026-11-02 (Bishal Ghimire), written down by the architect so every
later design can be checked against it. Decisions that follow from it are in
`docs/spec/adr/ADR-0011-emr-lite-share-a-case-and-deployment.md`; the ASD-specific plan is
in `docs/roadmap_2026q4/`. The older `docs/product_vision.md` describes the palm and flood
products and is unchanged.

## Mission

Help clinics in low-resource countries, Nepal first, where a child-development clinic
usually has **no electronic record system at all**: paper registers, a phone, and a
clinician's memory. The product is a low-friction helping hand, not a heavy medical-report
application: a modern, easy mobile app to collect observations, and a console to organise
them. Because nothing else exists in the clinic, the app and console together **become the
clinic's record** ("EMR-lite"): who was seen, when, what was observed, what the clinician
wrote, what happens next. It must be robust end to end, easy to run on one server, and must
let a clinician **share a case with a doctor or researcher abroad** (typically the US) when
they need a second opinion, without e-mailing spreadsheets.

## Who it serves

- **Clinic staff** (clinician, nurse, front desk) in a clinic with no IT department: they
  run the app on a clinic-owned tablet or phone and the console in a browser.
- **The child and parent**: a 30–60 s observation, consent in their language, their data
  stays in their clinic unless they agree to share it.
- **A doctor or researcher far away** who receives one case as a standard package (FHIR R4
  bundle, PDF, de-identified data) and needs no account on our system to read it.
- **The owner/operator** who installs and upgrades the server with one command and restores
  from a backup without an engineer.

## What it is

- A **mobile observation app** (iOS today; Android is an open question, see below) that
  records gaze-engagement sessions, scores them on the device with the shared Rust core,
  works fully offline and syncs when the network returns.
- A **console** that is the clinic's record: participant (patient) records, visits
  (encounters), observations from sessions, clinical notes, attachments, follow-ups,
  search, and a longitudinal view per child. "Clinical support" means helping clinicians
  record, review and follow up. It never means automated diagnosis (ADR-0004).
- A **share-a-case export**: one consented, audited package per participant (FHIR R4 +
  PDF + de-identified data), downloadable or handed over through a short-lived link.
- A **one-server deployment**: Docker Compose, TLS, automatic encrypted backups, a
  rehearsed restore, upgrades in one command.

## What it is not

- Not a diagnostic device and not a screening verdict. The attention flag is an
  observation aid; dispositions and impressions are the clinician's own words.
- Not a hospital-integration product. HL7 feeds, SFTP drops to hospital systems and
  SMART-on-FHIR launch are deferred: the clinics we serve have no hospital system to feed.
- Not a full EMR: no billing, pharmacy, lab orders, scheduling engine or insurance.
- Not a cloud SaaS that holds every clinic's data in one place. Each clinic (or a trusted
  operator for several clinics) runs its own server; cross-border movement of data is a
  deliberate, consented, audited act.

## Principles

1. **Low friction.** Fewer screens, sensible defaults, no configuration a nurse would not
   understand. If a feature needs a manual, it is wrong.
2. **Offline first.** Capture never depends on the network. Sync is automatic, resumable
   and visible ("3 sessions waiting"); nothing is lost when the connection drops mid-upload.
3. **Privacy by design.** No raw video, pseudonymous tokens on the wire, identity only when
   the clinic turns it on, encryption at rest and in transit, every read of clinical content
   audited, erasure that works. Security is designed to HIPAA's technical safeguards and
   Nepal's Privacy Act 2018; the legal and administrative parts (agreements, policies, risk
   analysis) are the owner's, not the code's (ADR-0011).
4. **Rust core everywhere.** Scoring, metrics, contracts, de-identification, export formats,
   sync and backend logic exist once, in Rust, and every platform calls that one
   implementation (`CLAUDE.md`, ADR-0006). Native code per OS does only UI, capture,
   permissions and platform glue.
5. **Share to the US bridge, not hospital plumbing.** One standard, portable case package
   replaces per-hospital integrations.
6. **Runnable by one person.** One command to install, one to upgrade, one to restore; the
   backup is encrypted and tested on a schedule, not assumed.
7. **Honest words.** Claims in the UI, store listing and documents are gated in CI
   (ADR-0004); what the product says it does is what it does.

## Open questions for the owner (not decided here)

- **Capture device**: clinic-owned iPad with TrueDepth (works today) versus an Android
  trial on cheap phones (front-camera gaze needs a feasibility spike before any promise).
- **Identity in the record**: whether a clinic without any other system may store a
  child's name and guardian contact in the console (opt-in, encrypted), see ADR-0011.
- The privacy decisions O-1..O-15 in `docs/security/dpia-lite.md`, which cross-border
  sharing and de-identification rules depend on.
