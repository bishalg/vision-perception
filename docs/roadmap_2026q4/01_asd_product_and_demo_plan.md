# 01 — ASD Gaze App: Product Reality and 3-Week Demo Plan

> Target demo window: **2026-11-10 → 2026-11-17**. Audience assumed: clinicians / partners / investors, not App Review.

## 1. What the app actually is today (Release build)

**Name:** "EarlyGaze Observation Diary" (placeholder, H-9). Bundle `org.curv.asd.gazecollector`. iOS 18+, iPhone/iPad with TrueDepth (enforced by `UIRequiredDeviceCapabilities`).

**Flow:** Mode selection (Parent & Caregiver / Clinic) → onboarding (child age bracket 12–60 mo, language, 30 s or 60 s battery, consent switch) → optional on-camera age-band check (inert without a model) → On-Demand-Resources download of the stimulus pack → positioning (distance 35–65 cm, yaw ≤35°, pitch ≤30°) → 5-point bubble calibration (affine) → 4 or 8 video clips → results.

**Measures (Swift, CurvAsdGazeKit):** ARKit `lookAtPoint`/eye transforms → screen raycast → 1-Euro filter → calibration → quality gate → I-VT fixations/saccades → dynamic AOI dwell (social / geometric / eye / mouth / orienting) → GeoPref social-preference score, dwell ratios, first-eye & orienting latency, gaze-cue accuracy, blendshape affect (smile intensity/reactivity, flat affect, surprise), head pose, distance, valid-frame ratio.

**Outputs:** per-clip expected-vs-recorded matrix against `normative_baselines.json` (10 clips × 5 age brackets, *provisional, literature-seeded, partly interpolated*) → weighted composite with 0.60 low-confidence gate → **traffic-light attention flag** (green ≥0.65 & <40 % atypical rows; red <0.35 or ≥40 % atypical; unreliable → yellow) with mandatory "screening signal, not a diagnosis" copy → pediatrician PDF (PDFKit) → FHIR R4 Bundle (Patient, RelatedPerson, Device, Observation, QuestionnaireResponse, DocumentReference, Provenance) → CareKit/ResearchKit-style JSON.

**Also in the app:** caregiver dashboard, mood diary, medication/care tasks (CareKitStore 4.1), milestones, reminders, Live Activity widget, Hindi/Nepali/English in-code localisation.

**Privacy posture (strong, keep it):** zero raw video; frames encrypted AES-GCM-256 JSONL with a Keychain-persisted key (docs that say "Secure Enclave" are wrong — fix docs, not code); file protection complete; pseudonymous `child-<id>` tokens; consent-gated capture; delete-all; `PrivacyInfo.xcprivacy` "Data Not Collected"; simulated data badged and refused at every export sink.

**Compiled out by default:** ML risk % / "Elevated risk" (Debug-only `ASD_RESEARCH_CLASSIFIER`, ADR-0004), M-CHAT-R/F (`MCHAT_LICENSED=1` only), research upload (needs `ASDResearchStudy` Info.plist with https DNS URL), simulation (`ASD_SIMULATION_ENABLED`, Debug only).

## 2. Pitch vs code gap (must be reconciled before the demo)

| Pitch claim (`docs/pitch_deck/*`) | Reality | Demo wording |
|---|---|---|
| "FDA-grade autism screening" | ADR-0004 non-diagnostic; claims gate in CI | "Objective attention observation that supports, not replaces, screening" |
| "16 digital biomarkers" | ~10 metrics + 4 affect scores; names being renamed `Biomarker*`→`Engagement*` (WP-4.4) | "Per-clip engagement metrics vs age-bracket expectations" |
| "C++20 perception core, 100 % coverage" | iOS path is pure Swift + ARKit; C++ pack unlinked | Don't mention C++ for the iOS app; mention the monorepo core only for flood |
| "18 languages" | en/ne/hi | "English, Nepali, Hindi" |
| "FHIR R4 + SMART, Epic/Cerner" | FHIR bundle export works; SMART client exists; no server | "FHIR R4 export ready for EHR import" |
| "21 CFR Part 11 audit trail", "SOC 2", "FDA Q-Sub Q4 2026", "Protocol #2026-088" | None exist | Remove |
| "30–60 s at home" | True | Keep |
| "Zero raw video, AES-GCM-256, Secure Enclave" | True except key is Keychain, not SE | "Zero raw video, encrypted on device" |

Action: WP-E.1 rewrites `docs/pitch_deck/*` to the right-hand column and adds a clearly-labelled "Where we want to be in 2027" section for aspirations.

## 3. Demo configuration (H-3 default: Release-equivalent, flag only)

Build a Release-configuration app with **one** extra Debug-style affordance: a hidden "Demo tools" screen (gated by `-DASD_DEMO_TOOLS`, never in store builds, checked by `check_release_symbols.sh`) that can:
- replay a pre-recorded **synthetic** session (not a child) so the results screens can be shown without a toddler present;
- toggle the backend endpoint between *off* and the demo server;
- show the sync queue state.

Everything else is the shipping path. Do **not** demo the research classifier or M-CHAT.

## 4. Demo storyline (8–10 minutes)

1. **Problem + privacy** (1 min): zero-video principle, on-device numbers only.
2. **Live capture on device** (3 min): calibration → 30 s Express battery with an adult volunteer (the age check hint "adult in front of camera" is a nice touch, if the model is trained; otherwise it stays silent) → results: per-clip matrix, flag, heatmap replay.
3. **Clinician outputs** (1 min): PDF share, FHIR bundle.
4. **Backend + admin panel** (3 min): the session appears in the clinic console within seconds; show cohort view, quality review queue, stimulus catalog version and baseline version that scored it, audit log entry. This is the new thing (WP-B.1–B.4, WP-C.1–C.2).
5. **Roadmap slide** (1 min): own norms from consented data, clinic RBAC, EHR.

## 5. What must be true by 2026-11-10

| # | Item | WP |
|---|---|---|
| D1 | Device checklist passed on a real iPhone (U7) | WP-A.1 |
| D2 | Known bugs fixed: double CareKit outcome write; delete-all second-prompt semantics; classifier double-run in Debug | WP-A.2 |
| D3 | History store moved off UserDefaults to protected file store (WP-4.3 in old tracker) — not strictly needed for demo; do if cheap | WP-A.3 |
| D4 | Sync client sends `Authorization: Bearer <device token>` + `X-Idempotency-Key` + `X-Schema-Version`; payload matches backend `POST /v1/sessions` | WP-A.4 |
| D5 | Backend v0 reachable at an https DNS host (Cloudflare Tunnel or deployed), ingests sessions, lists them | WP-B.1–B.4 |
| D6 | Admin console v0: login, sessions table, session detail (per-clip rows, flag, quality), review queue | WP-C.1–C.2 |
| D7 | Stimulus clips: either rights confirmed for internal demo, or replaced by own-generated clips with correct durations (H-8) | WP-D.1 |
| D8 | Pitch/demo deck re-based (§2) | WP-E.1 |
| D9 | `ios.yml` green on the demo commit | WP-X.2 |

Nice-to-have: trained age-band model from a cleared dataset (WP-D.3) so the adult-hint demo moment works; otherwise skip the moment.

## 6. Device checklist log

Fill in after U7. Scenario ids from `docs/spec/modules/asd-gaze-app-store-readiness/acceptance.md`.

| Scenario | Device / iOS | Result | Notes |
|---|---|---|---|
| 1 | | | |
| 10–16 | | | |
| 19 | | | |

## 7. Known issues to carry (from HANDOFF 2026-10-24)

- `MainGazeScannerView` writes the CareKit outcome twice.
- Debug: classifier runs twice per session.
- Delete-all: observation data already gone before the care-plan prompt.
- `RiskStratification.elevatedRisk` raw string compiles in all builds (claims-gate blind spot: scanner only checks `Text(`/`Label(` lines).
- `ResearchKitSyncServiceTests` / `ResearchKitStudyTests` teardown leaves defaults files.
- `xcodebuild test` occasionally hangs at launch.
- Docs drift: iOS 17 vs 18, Secure Enclave vs Keychain, "screenshots not committed" vs 12 committed, name "Autism & ADHD Test" in ASO SSOT vs "Observation Diary" in metadata.
