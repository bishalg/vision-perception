# Android parity: observation capture app, requirements and architecture (design record, 2026-11-08)

Owner decision (2026-11-08): the Android app mirrors the iOS store build one to one (capture,
consent, permission primers, per-feature opt-ins, Privacy & Your Data, pre-session disclaimer,
Sessions list, Settings), with the same wording, the same disclaimers and the same data
boundary. **No Android code is written now.** This record fixes requirements and architecture
so the work can start without a second design pass. It builds on AND-1..4 (ADR-0011 §7),
`apps/android_asd_core` (the AAR, done) and `apps/android_asd_gaze_spike` (throw-away, WP-A.14).

## 1. Non-negotiables inherited from the project

- Rust core, Kotlin shell (`CLAUDE.md`, ADR-0006). Scoring, quality, raw log, sync protocol,
  signing, config bundles, display bands: all through the UniFFI AAR `org.curv.asd:core`.
  Kotlin never re-implements a formula, a threshold, a schema or a sentence.
- Same contract: `docs/spec/schemas/asd/session-v1.8.json`, config-bundle-v1, i18n-v1.
- Same claims gate: `scripts/check_asd_claims.py` already scans Android store text
  (`fastlane/metadata/android`); it gains a Kotlin/Compose string scan (ANDP-14).
- Same data boundary as iOS decisions S-4/S-5: local-only until the clinic enrols the device;
  no camera frames stored; names never leave the device.

## 2. Requirements (ANDP-n, MUST unless marked)

**Parity**
- ANDP-1 Screen set equals the iOS store build: Consent flow, Permission primers, Before You
  Begin, Positioning, Calibration, Stimulus capture, Post-session (recording quality only),
  Sessions, Settings (Privacy & Your Data, About, Delete all). No dashboard, no verdict.
- ANDP-2 Every user-visible sentence comes from the shared catalogue (`asd-contract` i18n,
  `domain.<lang>.json`, en/ne/hi). Consent sections, primer copy, opt-in copy, disclaimers,
  the mandatory sentences (S-8) are catalogue keys, not Kotlin or Swift literals. iOS adopts
  the same keys in WP-S.6 so both platforms render identical text. `WP-L.5` generates
  `res/values(-ne,-hi)/strings.xml` from the catalogue.
- ANDP-3 Consent record is byte-identical across platforms: `asd-contract` exposes
  `consent_document_text(version, lang)` and `consent_document_sha256(version)` over the
  canonical English text; the device stores `{version, sha256, agreedAt, guardianName,
  relationship, appVersion, platform}` and a new version re-prompts before the next capture
  (S-10). The record is part of the session upload envelope (`consent` object, session
  schema v1.9) so the console can prove consent per session.
- ANDP-4 Per-feature opt-ins, all default off except the observation itself (S-12); the
  opt-in catalogue (`optin.<id>.{title,what,stored,never_stored}`) is shared.
- ANDP-5 Disclaimer surfaces as S-13, same eight places.

**Capture**
- ANDP-6 Capture backend is pluggable behind the spike's `CaptureBackend` interface:
  `MediaPipeBackend` (Face Landmarker: 478 landmarks, 52 blendshapes, facial transform
  matrix, iris) and `ArCoreBackend` (Augmented Faces: mesh and pose, no iris). Choice is
  the WP-A.14 number sheet outcome (ADR-0011 Q1). Gaze estimation from landmarks (iris
  centre relative to eye corners, head pose from the transform) is a Rust function
  (`asd-core::gaze::landmark_gaze`, new), fed per frame; Kotlin only moves tensors.
- ANDP-7 The raw log is `asd-raw-tracking/1` from Rust (`gazeRawLogDocument`), as the spike
  already does; `captureSource` = `androidMediaPipe` or `androidArCore` (contract enum
  addition) so the console and norms never mix it with TrueDepth data.
- ANDP-8 Calibration and validation layouts, target timing, quality gate and session verdict
  come from the installed config bundle via the AAR (`gazeDefaultConfigJson`,
  `session_quality_verdict`), as on iOS after WP-M.1.
- ANDP-9 Devices without a usable front camera or below the frame-rate floor (A.14 sheet)
  get the iOS `UnsupportedDeviceView` equivalent; the app still installs and shows Settings
  and Privacy (Play policy: do not require hardware the listing does not declare; use
  `<uses-feature android:name="android.hardware.camera.front" android:required="false">`).

**Privacy and platform**
- ANDP-10 Storage: sessions sealed by Rust (AES-GCM, same vault format) with the key in
  Android Keystore (StrongBox when available), `allowBackup=false`, `dataExtractionRules`
  excluding the vault and queue; no `WRITE_EXTERNAL_STORAGE`; no logs with biometrics.
- ANDP-11 Sync: `asd-sync` request builders through the AAR; WorkManager with network and
  battery constraints, exponential backoff, foreground-less; device token in Keystore-backed
  EncryptedSharedPreferences; enrolment by clinic code or Play Integrity (the App Attest twin).
- ANDP-12 Permissions: CAMERA (runtime, primed), POST_NOTIFICATIONS (API 33+, primed, only
  with the reminders opt-in), RECORD_AUDIO only with the dictation opt-in, READ_MEDIA_IMAGES
  only with the photo opt-in via the Photo Picker (no permission needed on API 33+; prefer
  it). No ACTIVITY_RECOGNITION (device motion through SensorManager needs none). No
  location, no contacts, no phone state, no AD_ID; `com.google.android.gms.permission.AD_ID`
  removed with a manifest `tools:node="remove"`.
- ANDP-13 Play Console: Data safety form generated from the same manifest as the iOS privacy
  label (one YAML in `docs/security/data-inventory.yml`, two renderers); Health apps policy
  declaration ("health and fitness" category, no medical claims); target API 35+;
  `isGame=false`; not a Families (child-directed) app because the operator is an adult
  guardian or clinic staff; age-rating questionnaire answered identically to iOS.
- ANDP-14 Gates: claims script scans `app/src/main/**/*.kt` string literals and
  `res/values*/strings.xml`; a release-symbol check greps the R8-mapped APK strings for the
  same forbidden needles as iOS; `detekt` and `lint` warnings as errors; JVM tests for every
  store/record class; Compose UI tests for the consent flow and the pre-session screen;
  golden parity test (already in the AAR) stays required.
- ANDP-15 Offline-first and no analytics SDK, no crash SDK unless the owner decides one for
  both platforms at once (same decision as WP-A.10).

**Explicitly out of scope for the capture app** (lives in the console or a later family app):
care plans, medication log, mood diary, milestones, PDF, FHIR, any band or flag display.
CareKit has no Android equivalent and none is needed; Health Connect has no State of Mind
type, so a future family app stores mood locally.

## 3. Architecture

```
┌──────────────────────── Kotlin (thin) ────────────────────────┐
│ Compose UI: Consent, Primers, BeforeYouBegin, Capture, Sessions│
│ ViewModels (StateFlow)  │ CameraX + MediaPipe/ARCore backend   │
│ Keystore, EncryptedPrefs│ WorkManager sync worker │ Photo Picker│
├──────────────── org.curv.asd:core (AAR, UniFFI) ───────────────┤
│ gaze: landmark_gaze (new), calibration, filter, quality, raw log│
│ consent: document text/sha (new) │ display: bands (unused here)│
│ sync: request builders, bundle verification, device auth      │
│ contract: schemas, i18n catalogue, claims terms                │
└──────────────────── Rust (shared with iOS/web) ───────────────┘
```

Module layout (`apps/android_asd_capture/`, new; the spike is deleted once A.14 is decided):
`app` (Compose, Hilt), `feature-consent`, `feature-capture`, `feature-sessions`,
`feature-settings`, `data-vault`, `data-sync`, `core-strings` (generated from the catalogue).
Each feature module depends on the AAR and `core-strings` only; no feature depends on another.

Threading: capture frames on a dedicated `HandlerThread`; one `GazeProcessor` actor
(Kotlin coroutine `Channel`, single consumer) owns the Rust session handle, mirroring the
iOS `GazeProcessingActor` and its ordering tests (CONC-1..4).

## 4. Work packages (WP-AN.n), order and dependencies

| WP | Tier | Scope | Depends on |
| --- | --- | --- | --- |
| WP-AN.0 | A (done here) | This record; owner decisions AN-D1..D4 below | – |
| WP-S.6 | S | iOS: move consent, primer, opt-in and disclaimer text into the catalogue; `consent_document_text/sha256` in `asd-contract`; session schema v1.9 `consent` object; iOS reads them through `asd-ffi` | WP-S.5 |
| WP-AN.1 | S | Rust: `captureSource` enum additions, `gaze::landmark_gaze` from MediaPipe landmarks with golden tests against the spike's adult logs; raw-log fields for landmark backends | A.14 sheet |
| WP-AN.2 | I | Project skeleton, modules, Hilt, catalogue-generated strings (WP-L.5), claims scan for Kotlin and `strings.xml`, CI job `android.yml` | WP-S.6 |
| WP-AN.3 | S | Consent flow, primers, opt-ins, Privacy & Your Data, Before You Begin (Compose), record store with Keystore | AN.2 |
| WP-AN.4 | S | Capture: backend adapter, positioning, calibration, validation, stimulus player (ExoPlayer, the same clip catalogue and licence gate), session seal, Sessions list | AN.1, AN.2 |
| WP-AN.5 | S | Sync worker, enrolment (clinic code, Play Integrity), delete-all, server-side erasure request | AN.4, backend erasure endpoint |
| WP-AN.6 | I | Play listing from `fastlane/metadata/android` (already rewritten), Data safety from the shared inventory, screenshots, internal testing track | AN.3, AN.4 |
| WP-AN.7 | I | Release-symbol check, detekt/lint as errors, Compose UI tests, device matrix run (3 phones from the A.14 sheet) | AN.3..AN.5 |

Owner decisions needed before WP-AN.1: **AN-D1** backend (MediaPipe vs ARCore, from the
A.14 sheet); **AN-D2** minimum device class and frame-rate floor; **AN-D3** publisher
account and package name (`org.curv.asd.capture` placeholder; H-6 twin); **AN-D4** whether
Android sessions enter norms (recommendation: no, flagged by `captureSource`, until a
parity study against TrueDepth exists).

Gates every WP-AN package passes: AAR golden parity, JVM unit tests, Compose UI tests,
claims scan, release-symbol check, `scripts/build_asd_android.sh --test`.
