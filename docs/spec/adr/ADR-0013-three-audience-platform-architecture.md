# ADR-0013: One platform for three audiences (researcher, clinician, family) on one Rust core; no hard-coded parameters; every string localised; tunable at run time

Status: Proposed, 2026-11-03 (owner statements the same day: "make a robust architecture
for this to be research as well as dr centric and client side alike in mobile to more
integrate tool for all - bridge the gap, make this our core vision"; "the main thing is
flexibility: nothing should be hard-coded, nor the user-interface strings, which should be
localized; and the researcher tool should be powerful enough to let them change and
fine-tune the knobs … and decide at run time itself")
Vision: `docs/product/VISION.md` (updated the same day). Detail: ADR-0012 (researcher
toolkit, phase 8), ADR-0011 (EMR-lite, share-a-case, deployment, phase 7), ADR-0008 (the
clinician role and exports), ADR-0007 (gaze robustness), ADR-0006 (Rust core), ADR-0004
(claims), ADR-0002 (consent and retention).
Packages: phase 8 WP-X.* (ADR-0012); localisation WP-L.0..L.5; mobile and family
WP-M.0..M.7; console WP-C.22; tracker `04 §10`.

## Context

Phases 1–7 built, in this order: a caregiver iOS app that captures gaze sessions and
scores them on the device with the Rust core; a backend and console for clinics; a
clinician role with review, sign-off, notes and exports; a record-keeping console
(encounters, attachments, follow-ups, participant search) with share-a-case to a remote
doctor or researcher; a one-command clinic server; Android bindings. Each phase served one
audience at a time. The owner's correction is that the product is **one tool for three
audiences at once**: the researcher who tunes and analyses, the doctor who records and
reviews, and the family who captures at home or in the clinic and should see something of
their own record. The gap is not a missing product; it is the seams between the three.

What exists per audience (verified at `ee82015`; details in the three surveys of
2026-11-03 recorded in ADR-0012's context and below):

- **Researcher.** Console: config editors with live Rust validation, a sandbox re-score,
  norm runs with promotion, de-identified dataset exports (JSONL/Parquet), audit CSV;
  `researcher` role = `ConfigRead`, `ConfigWrite` (unused), `JobRun`, `ExportRead`,
  `ExportDeidentified`. No experiments, no cohorts, no frame-level reprocessing, no
  parameter registry (ADR-0012).
- **Clinician.** Console: worklist, clinical review with hash-bound signatures, encrypted
  notes, participant workspace and timeline, encounters, attachments, follow-ups due,
  share-a-case with passcode links; PDF without Devanagari (WP-I.5 open); letters
  re-scoped (WP-C.12). Mobile: nothing; the iOS app has no user login (device tokens
  only, `DeviceRegistrationService`), reads nothing from the record (only the config
  bundle), and "clinic mode" is an `AppMode` switch that changes copy and shows a sync
  line.
- **Family.** iOS: onboarding with PIA consent and one child profile, dashboard with care
  tasks (CareKit), diary, milestones, M-CHAT, observation session with calibration,
  results with `AttentionFlag`, history, settings, research enrolment with ResearchKit
  consent and the norm-collection opt-in, offline queue. The caregiver has no account and
  no identity on the server beyond `participants.token` (`child-<uuid>` per profile since
  WP-A.16a); the device never reads its own record (follow-ups, visits, notes); no
  reminders from the clinic; no messaging. Android: bindings and a feasibility spike, no
  app.
- **Localisation.** Console: ~1,420 string entries in `src/messages/*.ts`, English only,
  `<html lang="en">` fixed, no locale selection, enum labels mostly mapped, a few raw
  (`language`, `age_bracket_at_enrol`, `capture_source`, job `error`), Rust-produced
  texts shown as sent (validation messages, flag headline/detail, problem `detail`), no
  lint rule. iOS: `AppLocalizer` with 143 keys in en/ne/hi, but 220 `Text("…")` and ~144
  other literal calls in 32 files bypass it; no String Catalog; `developmentLanguage: en`;
  model types carry English labels (`"12–17 mo"`). Android spike: literals. Rust: every
  export text English; no `locale` on `ExportRequest`; PDFs base-14 fonts, no Devanagari;
  problem `detail` English; no `locale` on `organizations.settings` or `users`.
- **Hard-coded parameters.** Besides the asd-core constants ADR-0012 lists: iOS result
  views carry ~20 traffic-light cut-offs (0.65/0.35, 60 %/50 %, 550/750 ms, 20 %/10 %,
  75 %), three different age-band splits (`ChildProfile`, `OnDemandStimulusManager`,
  milestones), `maxFrameDwellSec 0.1`, positioning timings; the console's `AGE_GROUPS`
  labels and `DriftChart` without the profile's lines; backend review thresholds
  (`0.80`, `0.60`) as constants. Operational limits (TTLs, rate limits, batch sizes,
  share-link bounds) are constants or env and are not analysis knobs.
- **Runtime configuration.** Devices pull a signed bundle (`GET /v1/config/bundle`, ETag,
  Ed25519, rollback refusal, expiry; WP-A.5) and install it at the next launch; the
  console validates and re-scores through `asd-wasm`; publishing is four-eyes under the
  vendor role.

## Decisions

### A. One platform, three audiences, one core

1. **One product, three surfaces per audience, one backend, one Rust core.** The
   audiences and their surfaces are:
   - **Researcher**: the console research area (ADR-0012: registry-driven experiments,
     single-session live preview in WASM, cohort runs, sweeps, age profiles, reproducible
     runs), de-identified exports (dataset, analysis results, case package in
     de-identified mode), `asd-cli` on their own machine for the same maths over exported
     logs they are allowed to hold. Never clinical text, never identity.
   - **Clinician/doctor**: the console record workspace (ADR-0011), clinical review and
     sign-off (ADR-0008), share-a-case, printable summary; and on mobile a **clinician
     companion** that is, in order: (a) the console itself, made responsive and
     installable as a PWA on the clinic iPad (WP-C.22: worklist, follow-ups, participant
     search, visit entry and case read-out at tablet width; no new code paths, same RBAC,
     same audits), then (b) native companion screens in the iOS and Android apps for the
     parts that must work offline in a clinic corridor: visit entry and follow-up
     completion queued through `asd-sync` (WP-M.2), behind an org-user login on the device
     (WP-M.0). The native companion never re-implements a console screen that works in
     the PWA.
   - **Family (parent/caregiver)**: the iOS caregiver app as it is (capture at home or in
     the clinic, care tasks, diary, consent) and its Android equivalent when the capture
     spike says go (WP-M.5); plus **what a family may see of their own record** (WP-M.3):
     upcoming follow-ups and visit headers for their child, the clinic's contact line,
     and the non-diagnostic observation summary they already get on the device, read
     through the device plane for the participant tokens that device minted or scanned.
     **Follow-up reminders** are in scope as local notifications from the synced
     follow-ups (WP-M.4). **Messaging is out of scope** for this phase (open question 2):
     it is a second clinical-text channel with its own retention, audit and safety rules
     and the clinic has a phone.
2. **Integration: every artefact has one owner and flows through the backend; sync is
   `asd-sync`, offline-first on every mobile surface.** A **session** is created on a
   device (family or clinician-held), scored on the device, posted idempotently
   (ING-*), optionally with a research frame log (ADR-0012 RFL-*), and lands on the
   participant the token names (REC-11/12). A **visit (encounter)** is created in the
   console by the front desk or clinician, or offline on the clinician companion
   (WP-M.2) and synced through the same idempotent request builder (`asd-sync` gains
   `build_encounter_request`); sessions link to it. An **attachment** is uploaded from
   the console or the companion (REC-7 proxy route, `Content-Digest`), encrypted under the
   org DEK. **Consent** is captured on the device (PIA consent for capture, research
   consent and the norm opt-in; O-6 owns the text and evidence) and travels in the
   session (`normCollection`), and in the console as a consent reference for sharing
   (SHR-7); a withdrawal from the device (care events erasure request, WP-B.6) or the
   console (privacy page) stops research use and triggers erasure. **Configuration**
   flows one way: console (draft → approve → publish → signed bundle) → devices at
   launch (D7 below) and the console's own `config/effective`. The remote US doctor or
   researcher is reached only by **share-a-case** (passcode link, audited) and by
   **research exports**; they never get an account on the clinic server (ADR-0011 D5/D6).
   The Nepal single-server deployment (DPL-*) hosts all of this; nothing in this ADR adds
   a service.
3. **Shared Rust core on every OS; thin native UI.** The crate map per surface:

   | Surface | Crates | Binding |
   | --- | --- | --- |
   | iOS caregiver app and clinician companion | `asd-core` (gaze pipeline, metrics, scoring, flag, quality, params), `asd-contract` (session, config, registry, EMR DTOs, i18n catalogue), `asd-sync` (requests, retries, bundle acceptance, 409 classification), `asd-export` (local FHIR/PDF, `pdf` feature off until WP-I.5) | `asd-ffi` (UniFFI, `AsdCore.xcframework`) |
   | Android caregiver app and companion | the same four | `asd-ffi` (UniFFI Kotlin, AAR `org.curv.asd:core`) |
   | Console | `asd-core`, `asd-contract` (validation, registry, rescore, reprocess) | `asd-wasm` |
   | Desktop | the console in a browser or PWA; Tauri 2 remote shell when a binary is needed (ADR-0011 D10) | none (loads the console) |
   | Backend | `asd-core`, `asd-contract`, `asd-export`, `asd-sync` (server side of the protocol) | in-process |
   | CLI | all | in-process |

   Rules (from `CLAUDE.md`, restated as MUSTs): no platform re-implements maths,
   validation, de-identification, formats or protocol logic; Swift, Kotlin and
   TypeScript hold UI, capture, permissions, keychains, notifications and platform glue;
   a native file that computes a metric, a threshold decision or a format is a defect.
   **One contract**: every wire shape is a Rust type exported to `docs/spec/schemas/asd/*.json`
   by `asd-cli schema export`, checked in CI, and consumed by the console's zod tests and
   the apps' fixtures; the parameter registry and the domain string catalogue join that
   set.
4. **Identity and roles across audiences.** Three identity planes, deliberately
   different:
   - **Org users** (console and, with WP-M.0, the companion): `users` with roles
     `owner, admin, clinician, researcher, reviewer, viewer` per org and the vendor
     `superadmin`; password + TOTP; RBAC matrix (RBAC-*), vendor excluded from clinical
     content; every read of clinical content audited (HIP-1).
   - **Devices**: enrolled with a one-time code into one org, long-lived hashed device
     token, device plane only (`/v1/sessions`, blobs, care events, config bundle, and the
     new participant-scoped reads of WP-M.3); a device is the family's credential today
     and stays so.
   - **Caregivers/families**: **no account today**; identity is the device plus the
     participant token(s) that device minted (`child-<uuid>` per child profile) or scanned
     from the console (REC-12). A family's reads (WP-M.3) are scoped to those tokens and
     to nothing else; the server learns no name through this path. A **family account**
     (log in on a new phone, see the same child, share with the other parent) is a
     separate, owner-gated decision (open question 1): it needs verifiable parental
     consent (O-6), a credential recovery path a clinic can operate, and it depends on
     the identity decision of ADR-0011 Q3 (WP-B.21/C.20) because an account without a
     stored name is a token, which the device already is. Recommended default: not in
     phases 8–9; a second device joins a child by scanning the console QR (REC-12), which
     covers the two-parent case without accounts.
   - **The clinician on a device** (WP-M.0): an org user logs in on the app with the
     user plane (password + TOTP, the same `auth/*` endpoints; the token lives in the
     Keychain/Keystore with a 15-minute access token and the rotating refresh family;
     the device token stays for capture). The companion shows only what the role allows;
     the protocol logic (`build_login_request`, refresh rotation, step-up) goes into
     `asd-sync::user_session` so iOS and Android share it.
5. **Privacy boundaries: who sees what.**

   | Data | Family (device) | Clinician (console/companion) | Researcher (console) | Remote recipient | Vendor (`superadmin`) |
   | --- | --- | --- | --- | --- | --- |
   | Session metrics, flag, quality | own child, non-diagnostic wording (ADR-0004) | yes, audited | yes (no clinical text) | in the case package | yes |
   | Clinical text (summaries, notes, impressions) | no (summaries are the clinician's words to the record, not to the parent, until the owner decides otherwise) | `ClinicalContentRead` only, audited | no | identified package only, with consent reference | no (RBAC-3) |
   | Identity (MRN; name if ADR-0011 Q3 = yes) | own child, on the device only | `PatientRefRead`, audited | no | identified package only | no |
   | Research frame logs | produced only with research consent | no | `FramesRead`, audited; never exported | no | no |
   | Aggregates and distributions | no | `AnalysisRead` | yes, small-cell suppressed (`k`, PRV-1) | de-identified export only | read only |
   | Follow-ups and visit headers | own child (dates, kind, clinic line; no clinical text) | yes | no | identified package | headers |
   | Audit log | no | owner/admin | no | no | yes |

   Rules: de-identification lives in `asd-export::deid` and is the only path out for
   research (XC-4/5, PRV-3); small-cell suppression applies to every aggregate (PRV-1);
   families see non-diagnostic wording with the mandatory sentence on every result
   surface (CLM-*); every read of clinical content or participant detail writes an audit
   row (HIP-1), including the family's own reads (`participant.self_read`, device actor).

### B. Three platform principles (owner, 2026-11-03)

6. **No hard-coded parameters (PRM-*, INV-*).** Every algorithm constant (filter,
   fixation, saccade and blink thresholds, AOI margins, quality-gate limits, age bands,
   norm cut-offs, flag rules, display bands, review thresholds) lives in a versioned,
   signed configuration document from `asd-contract`, described by the self-describing
   **parameter registry** (ADR-0012 D1: id, type, unit, allowed range, default,
   description key, stage, and whether a change needs reprocessing from raw frames or
   only re-scoring). The console, iOS and Android render tuning and display from the
   registry and the effective configuration generically (no per-knob UI code); the core
   validates against it; `asd-cli params` checks any profile. The **migration
   inventory** of today's constants is `technical.md §2` of the research toolkit
   module (INV-1): asd-core rows move in WP-X.1, backend rows in WP-X.2, console rows in
   WP-X.5/X.7, iOS rows (display bands, age-band labels, dwell cap, the three age splits)
   in WP-M.1, which also adds a grep gate in `ios_test.sh` for numeric literals in
   `Engagement*Views.swift` and `*ResultView*.swift`. Operational limits (token TTLs,
   rate limits, batch sizes, share-link bounds) are not analysis knobs: they stay
   constants or `ASD_*` environment settings, listed in `rust/README.md`, and the
   registry test ignores them by construction (they are not in a config document).
7. **Every user-facing string is localised, on every surface, English and Nepali
   (`ne-NP`) first.** Rules:
   - **Domain vocabulary once, in Rust.** Labels for enumerations that cross surfaces
     (visit kinds, attachment kinds, care task categories, roles, review reasons, flag
     names and headlines, quality and tracking verdicts, scoring statuses, export
     formats, problem slugs, registry labels and help) are **translatable keys** in a
     shared catalogue `asd-contract::i18n` (`domain.en.json`, `domain.ne.json`,
     `include_str!`, keyed `kind.value`, e.g. `encounter_kind.screening_visit`,
     `param.metrics.ivtVelocityUnitsPerSec.label`, `problem.case-consent-missing`).
     `asd-cli i18n export --target console|xcstrings|android` generates
     `src/messages/domain.ts`, `Domain.xcstrings` and `values(-ne)/domain_strings.xml`;
     `asd-cli i18n check` fails CI on drift or on a key missing in one locale. Rust
     code that produces user-visible text (validation problems, flag headline/detail,
     problem `detail`, PDF/README text, CSV data dictionary) returns **keys plus
     parameters** (`ProfileProblem { path, key, args }`, `Problem { slug, params }`)
     and the surface renders them; where Rust must render final text (PDF, README,
     exported manifests) it takes a `locale` argument and resolves the same catalogue.
   - **UI chrome per surface, in the surface's native catalogue**: console
     `src/messages/*.ts` grows a `ne` tree with the same shape (type-checked: `Messages`
     type shared), locale chosen per org (`organizations.settings.locale`, default `en`)
     and overridden per user (`users.locale`), `<html lang>` and date/number formatting
     follow it; iOS moves to a String Catalog (`Localizable.xcstrings`, en/ne/hi) and
     `String(localized:)`, folds `AppLocalizer` into it, in-app override in Settings;
     Android `res/values(-ne,-hi)/strings.xml`. Model types never carry display strings
     (`PediatricAgeBracket` raw values become ids).
   - **Exports and the recipient's language**: `ExportRequest.locale` (default the org
     locale; the share-a-case dialog offers `en` for a US recipient), the README of the
     case package is rendered in the requested locale with the English mandatory
     sentences always present; Devanagari in PDFs is WP-I.5 (fonts, ADR-0008 Q3), until
     then the PDF stays English and the README is bilingual.
   - **The gate**: console `eslint-plugin-react` `react/jsx-no-literals` (already in
     `eslint-config-next`'s plugin set, no new dependency) with `noStrings: true,
     ignoreProps: false, allowedStrings: ["…", "·", "—"]` plus a small custom rule for
     `aria-label`, `title`, `placeholder` and `alt` literals; iOS and Android: `asd-cli
     i18n lint --swift DIR --kotlin DIR` (the port of the literal scan the claims
     scanner already does, so WP-D.8's port starts here) flags `Text("…")`, `Label("…")`,
     `Button("…")`, `.navigationTitle("…")`, `.accessibilityLabel("…")`, `alert("…")`
     and Kotlin `Text("…")`/`setText("…")` with a literal that is not a `String(localized:)`
     or a resource reference; the gate starts in warn mode with a baseline file and flips
     to fail per directory as WP-L.2 clears it. Packages: WP-L.0 catalogue and tooling,
     WP-L.1 console, WP-L.2 iOS, WP-L.3 Nepali review (owner + native speaker), WP-L.4
     Rust-rendered text and exports, WP-L.5 Android (with the app).
8. **Runtime tunability without a rebuild or a deploy.** A researcher changes knobs in
   the workbench and sees the effect in-session (asd-wasm live preview on one session,
   a cohort job for many), saves drafts as named **experiments**, and promotes one to a
   draft profile; clinical use changes **only** through publish-with-approval (GOV-*).
   Devices pick up published bundles through the existing WP-A.5 path, with these
   staleness rules (CFG-4, implemented in `ConfigBundleService` and the Android
   equivalent): refresh at launch and on foreground when the cached copy is older than 1
   hour; a connected device is therefore never more than **24 hours** behind a publish
   (`ASD_BUNDLE_MAX_STALENESS_HOURS = 24`, the console shows adoption lag per device from
   `bundle_adoption`); an offline device keeps its cached bundle until the bundle's own
   `expires_at` (set at publish, default 90 days), then falls back to the bundled copy
   and shows "configuration may be out of date" in clinic mode; every session records
   the `bundleVersion` it was scored with (exists), so a run can filter by it. The
   console and the backend read the effective configuration live (`config/effective`);
   the backend's review rules and the norm job read the published profile at run time
   (INV-1 backend rows).

### C. Phasing: what exists, what is in flight, the gaps in priority order

| Order | Gap | Package(s) | Tier | Size | Depends on |
| --- | --- | --- | --- | --- | --- |
| 1 | Researcher toolkit: registry, tunable gaze params, research frame log, live preview, cohort runs, suppression, exports | WP-X.0..X.10 (ADR-0012, phase 8 waves 0–2) | S/I | 3 waves | — |
| 2 | Shared domain string catalogue + `asd-cli i18n export/check/lint` + console `ne` and locale selection + literal gates | WP-L.0, WP-L.1 | S, I | I, L | — (L.1 after L.0) |
| 3 | iOS display bands, age labels and remaining constants from the bundle; grep gate | WP-M.1 | I | I | X.0 (profile 1.2.0 `display` section) |
| 4 | Console responsive + PWA (clinician companion step a) | WP-C.22 | I | I | — |
| 5 | iOS String Catalog migration (en/ne/hi), in-app language override | WP-L.2 | I | L | L.0 |
| 6 | Family's own record on the device: `GET /v1/participants/me/*` (device plane, token-scoped), follow-ups and visit headers screen, `participant.self_read` audit | WP-M.3 | S | I | B.19 (done) |
| 7 | Follow-up reminders as local notifications from synced follow-ups | WP-M.4 | I | S | M.3 |
| 8 | Org-user login on the device (user plane in `asd-sync::user_session`, Keychain/Keystore, TOTP) | WP-M.0 | S | I | — |
| 9 | Native clinician companion: offline visit entry and follow-up completion through `asd-sync` | WP-M.2 | S | L | M.0, C.22 |
| 10 | Rust-rendered text localised (`ExportRequest.locale`, keys + params for problems and validation, README per locale) | WP-L.4 | I | I | L.0 (fonts: I.5) |
| 11 | Sweeps and age profiles | WP-X.9 (phase 8 wave 3) | S | L | X wave 2 |
| 12 | Android caregiver app (capture + sync + results) on the AAR, after A.14 says go | WP-M.5 | S | XL | A.14 (owner Q1), L.0 |
| 13 | Android strings and the companion screens in the same app | WP-L.5, WP-M.6 | I, S | I, L | M.5, M.0 |
| 14 | Nepali review by a native speaker (owner) | WP-L.3 | owner + I | S | L.1, L.2 |
| 15 | Family account (owner-gated) | WP-M.7 | A + S | L | ADR-0011 Q3, O-6, open question 1 |

Items 1–5 are one quarter for a solo owner with agents; 6–10 the next; 11–15 as the
owner's answers arrive. Everything in flight from phase 7 (B.21, C.20, A.16b, E.8, A.14)
keeps its place.

## Alternatives rejected

- **Three apps (research, clinic, family).** Three logins, three sync clients, three
  stores; the owner's point is the opposite. One app per OS with role-gated surfaces,
  one console, one backend.
- **A separate "patient portal" web app for families.** Families in the target clinics
  use a phone, not a laptop, and have no account; the record they may see is small
  (follow-ups, visit headers, their own observation). The device plane already carries
  their identity. Revisit with the family-account decision.
- **Family accounts now.** Needs verifiable parental consent, recovery, and the identity
  decision; the QR join covers the second phone. Deferred, owner-gated.
- **In-app messaging between clinic and family.** A clinical-text channel with its own
  retention and safety rules; out of scope, owner question.
- **Native clinician screens first, PWA later.** The console already has every clinician
  screen; making it responsive is a week, a native copy is a quarter. Native only for
  offline visit entry.
- **A translation service or runtime translation.** Clinical and research vocabulary
  must be reviewed, versioned and testable; keys plus catalogues in git, exported by the
  CLI, reviewed by a native speaker (WP-L.3).
- **Letting Rust format final strings for the UI.** Puts every locale into the core
  crate's binary on the device and the wasm; keys plus parameters keep the core small
  and the surfaces in charge of layout, while the catalogue stays single-sourced.
- **A per-surface parameter file ("config.plist", "constants.ts").** It is a hard-coded
  constant with a different filename; every knob must be in the signed, versioned,
  registry-described document or it cannot be tuned, audited or reproduced.

## Consequences

- `asd-contract` grows two cross-cutting modules (`params`, `i18n`) and both are schema
  and CI gates; `asd-cli` grows `params` and `i18n` subcommands (the claims scanner's
  literal scan starts moving to Rust here; WP-D.8).
- `organizations.settings.locale`, `users.locale`, `ExportRequest.locale` are additive
  contract changes (migration-free: settings is JSONB; `users.locale` needs a column in
  the next free migration after 0027, or a settings-style JSONB column, decided by WP-L.1).
- The console gets a locale provider and a lint rule that will initially flag the known
  leaks (16 `aria-label` files, `SessionsTable` "Yes/No", `UsersAdmin` status text);
  WP-L.1 clears them.
- The iOS app gets a String Catalog and ~360 literal call sites to migrate (WP-L.2 is the
  largest localisation package); the app's `developmentLanguage` stays `en`.
- The device plane gains its first participant-scoped **reads** (WP-M.3); they are
  token-scoped, audited with a device actor, and carry no clinical text; the threat model
  gets a row.
- The apps gain a second credential (org user) when WP-M.0 ships; the device token is
  unchanged.
- The vision statement changes (VISION.md); ADR-0011's scope is unchanged and gains the
  mobile companion and family-view items as later packages.

## Open questions for the owner (recommended default in brackets)

1. **Family account** (log in on any phone, two parents, recovery) [no in phases 8–9;
   second device joins by QR; revisit with ADR-0011 Q3 and O-6].
2. **Messaging / chat between clinic and family** [out of scope; local reminders from
   follow-ups only (WP-M.4); the clinic phones].
3. **What a family may see of the record** [follow-ups, visit headers, clinic contact,
   their own non-diagnostic observation summary; no clinician text].
4. **Locale selection** [org default `en` or `ne` set at install; per-user override;
   device follows the OS language with an in-app override].
5. **Bundle staleness** [refresh at launch/foreground when > 1 h old; connected devices ≤
   24 h behind; offline until the bundle's `expires_at` (90 days), then fallback with a
   notice].
6. **Nepali reviewer** [a clinician who reads Nepali reviews `domain.ne.json` and the
   app catalogue before the pilot; WP-L.3].
7. **Devanagari fonts** (ADR-0008 Q3, unchanged) [ratify SIL OFL-1.1 for Noto Sans
   Devanagari; unblocks WP-I.5 and Nepali PDFs].
8. **Clinician companion device** [the clinic iPad with the console PWA first (WP-C.22);
   native offline screens (WP-M.2) after one clinic asks for corridor use].
9. **Android app go/no-go** (ADR-0011 Q1, unchanged) [decide on the A.14 number sheet;
   until then Android stays bindings + spike].
10. **Literal-string gate strictness** [warn with a baseline now; fail per directory as
    each package clears it; fail everywhere by the end of WP-L.2].
11. **Where the locale is stored** (added 2026-11-04 with the WP-L.0/L.1 design,
    `docs/spec/modules/asd-localisation/`) [typed columns in migration 0028:
    `users.locale` (nullable override) and `organizations.default_locale` (default `en`),
    instead of `organizations.settings.locale`; negotiation user > org > `Accept-Language` > `en`].
12. **Digits in the Nepali UI** [Latin digits (`ne-NP-u-nu-latn`) for measurements, dates,
    ids and counts on every surface; no Devanagari numerals in phase 8].
13. **Calendar** [Gregorian only; a Bikram Sambat display can come later as an org setting].
14. **Server-rendered text in L.1** [deferred to WP-L.4: the console shows the catalogue text
    for each problem slug and the English server `detail` as a secondary line; PDFs, letters
    and export READMEs stay English until WP-L.4 and WP-I.5].
15. **Hindi in the shared catalogue** [optional and partial (`domain.hi.json`), exported only
    to iOS/Android, falling back to English; the console ships `en` and `ne` only].
16. **Inconsistent iOS cut-offs** (WP-M.1 inventory) [one band per metric, the risk view's
    values: social preference ≥ 60 % / ≥ 50 %, eye dwell ≥ 25 % / ≥ 18 %, first-eye latency
    < 550 ms / < 750 ms; the detail view's 650 ms and the dashboard's "> 50 %" adopt them].
17. **Publishing a 1.3.0 profile** (with `display`/`sessionQuality`) [hold until an app
    release carrying WP-X.0c's tolerant bundle acceptance is on the pilot devices; older apps
    refuse unknown profile sections and keep their previous profile].
18. **Research-build Swift heuristics** (`EngagementClassificationEngine` weights, logit and
    cut-offs under `ASD_RESEARCH_CLASSIFIER`) [stay Debug-only and exempt from the WP-M.1
    gate; delete them the next time they are touched rather than port them; never in Release].
19. **Age entry on iOS** [stays band-based; the band's `representativeMonths` (14, 20, 28,
    40, 54 by default) is sent as `ageMonths`, so sessions do not change; exact birth month
    comes with the identity decision (ADR-0011 Q3, WP-B.21)].
20. **Devanagari web font in the console ahead of Q7** [ship Noto Sans Devanagari (SIL
    OFL-1.1) self-hosted in the console with WP-L.1; Q7 still governs fonts embedded in PDFs].
