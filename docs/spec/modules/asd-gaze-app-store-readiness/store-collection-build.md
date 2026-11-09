# Store build: observation capture only (design record, 2026-11-08)

Owner decision (2026-11-08): the App Store build of the iOS ASD gaze app is positioned as a
**clinic observation capture tool**. It records a gaze observation session and keeps or
delivers it for review on the clinic's platform (Curv Console). It shows no score, band,
flag, norm comparison, questionnaire result or report on the device. Analysis, reports and
exports live on the console (`docs/roadmap_2026q4/03_asd_admin_panel_spec.md`).

Wording rule: "observation capture", never "research", "study", "screening", "data collection
for research". Apple treats health research as human-subject research (guideline 5.1.3) and
asks for ethics approval; a clinic capture companion enrolled with a clinic code does not.

## Decisions

| # | Decision | Why |
| --- | --- | --- |
| S-1 | One compile flag `ASD_STORE_BUILD`, defined in the `Release` configuration only. Debug and DemoRelease are unchanged. | Keeps the 2026-11-10..17 demo build (DemoRelease) with its result screens. Hiding is UI-only, so it lands in a day; deleting the care-plan/CareKit code is a later package. |
| S-2 | Store build tabs: Capture, Sessions, Settings. No Dashboard, no Parent/Clinic mode picker (mode is set silently to `.clinic`). | The dashboard tab is the home of every verdict surface (M-CHAT row, trend chart, milestones, care plan). |
| S-3 | Post-session screen shows only: scanpath replay hero, recording quality (usable samples, quality verdict), where the session is (saved on device / queued / uploaded), New Observation, Done, and the not-diagnostic footer. | A quality grade is operational feedback for the operator, not a clinical result. |
| S-4 | Upload stays config-driven exactly as today (`ASDResearchStudy` plist dict; absent in the default build). Default store build is local-only; privacy manifest and label stay "Data Not Collected". | Backend is v0. When a clinic config ships, PRV-1 applies and the label changes to Health data linked to device. |
| S-5 | Capture still runs the kit scoring and stores the full session (vault, history, sync payload incl. attention flag). Only the display is suppressed. | The console needs the data; the device simply does not interpret it. |
| S-6 | Settings in the store build: calibration and camera check, child profile, language, sensor diagnostics, privacy and security (delete all), about with Privacy Policy and Support links and the disclaimer. Hidden: mode, medical conditions, care alerts, HealthKit mood. | Everything hidden is care-plan or diary territory that belongs to the clinic platform. |
| S-7 | Working name "EarlyGaze" (display name and store name); description calls it "EarlyGaze Observation Capture". Placeholder until H-9. | The tree had three names ("Eye Gaze", "EarlyGaze Observation Diary", "ASD Gaze"). "ASD" never appears in user-facing text. |
| S-8 | Mandatory sentences everywhere a session or the app is described: "This is not a diagnostic tool." and "It does not measure, screen for or diagnose any condition; your clinic reviews each session." | ADR-0004. |
| S-9 | Category stays HEALTH_AND_FITNESS (H6 unchanged). URLs stay on the `curv.engineering` placeholder until H-6; the same URLs are shown in-app so the listing and the app agree. **Amended 2026-11-09:** listing and in-app links moved to the interim host `open-spectrum-tracking.vercel.app` (`public-site-alignment.md` section 7); the support email stays on `curv.engineering` until H-6. | Changing category is an owner decision; a mismatch is not a rejection reason. |
| S-10 | Consent is a ResearchKit-style visual consent flow written in SwiftUI (sections with icon, summary and "Learn more", then a review page and typed guardian name with relationship). The real ResearchKit package is not added today. The record stores document version, SHA-256 of the document text, timestamp, guardian name and relationship. A new document version re-prompts before the next capture. | Owner request 2026-11-08: "consent screen from ResearchKit and CareKit should be used". The tree's `ORKConsentDocument` is a home-grown struct; the Apple package is UIKit and would add risk today. Avoid "research" framing (5.1.3). |
| S-11 | Every system permission gets an in-app primer screen first (camera, motion, notifications, speech and microphone, photos, HealthKit, age range): what is used, why, what is never stored, and that it can be changed in iOS Settings. "Not now" is always available and the app keeps working with the feature off. | Apple HIG "Requesting permission"; guideline 5.1.1(ii), (iv). |
| S-12 | Per-feature opt-ins, all default off except the camera observation itself (which the consent covers): age-range check, clinic delivery (enrolment), notifications, HealthKit, dictation, photo attachments. **Amended 2026-11-08 (WP-S.5 phase 1 finding):** facial-expression measurements are part of the observation and are listed under "What is recorded" in the consent, not offered as a toggle, because the kit records them unconditionally today; a toggle that does not gate anything would be an untrue statement to the guardian. WP-S.5b may reintroduce it once the kit pipeline can drop blendshape metrics per session. Every remaining toggle that does not yet gate its feature says so on its detail page. Capture runs with any subset. A "Privacy & Your Data" screen in Settings shows each toggle, the iOS permission status, how many sessions are on the device, whether the device is enrolled, what is never collected (camera images, names off the device, no analytics, no ads, no tracking), the signed consent record, delete-all, Privacy Policy and Support. | Transparency per surface; 5.1.1(i), 5.1.3(i); owner request "opt-in for every feature". |
| S-13 | The not-diagnostic disclaimer appears on: first launch, consent overview, the "Before you begin" pre-session screen, the post-session screen, the Sessions tab footer, Privacy & Your Data, About, and in every share or export text. The pre-session screen also states: "Only a qualified professional, using the clinic's analysis tools, can interpret an observation." | 1.4.1; owner request. |
| S-14 | Listing locales: the first App Store submission lists **en-US only** (owner, 2026-11-09). The hi and ne-NP listing texts are parked in `apps/ios_asd_gaze_app/fastlane/metadata_pending/{hi,ne-NP}` (not uploaded; `update_metadata` passes `languages: ["en-US"]`), still checked by `fastlane validate_metadata` and `scripts/check_asd_claims.py` with the same rules, and kept in sync (URLs). They move back to `fastlane/metadata/` when native hi/ne review and WP-S.6 land. The app's own localisation (AppLocalizer, consent text, i18n lint) is unchanged. | English is the only locale with reviewed copy; the in-app consent is English in every locale until WP-S.6. |

## Work packages (2026-11-08)

| WP | Tier | Scope | Files owned |
| --- | --- | --- | --- |
| WP-S.1 | senior | Flag, routing, Sessions tab, result and settings trimming, legal links, tests, release-symbol needles | `project.yml`, `Config/Info-Additions.plist`, `Sources/App/AsdGazeApp.swift`, `Sources/Views/GazeScreeningWizardView.swift`, `GazeSessionResultView.swift`, `SettingsView.swift`, `GazeStimulusScreeningView.swift`, `MainGazeScannerView.swift`, new `Sources/Models/StoreBuild.swift`, new `Sources/Views/StoreSessionsView.swift`, `Tests/BuildConfigurationTests.swift`, new `Tests/StoreBuildTests.swift`, `scripts/check_release_symbols.sh`, regenerated `project.pbxproj` |
| WP-S.2 | impl | Branding and copy: remove "ASD" from UI, taglines, verdict-like lines that survive in Debug/Demo, onboarding and consent copy, README | `SplashScreenView.swift`, `CareKitDashboardModals.swift`, `AppModeStore.swift`, `AppModeSelectionView.swift`, `AppLocalizer.swift`, `MilestoneDetailView.swift`, `ClinicalDevelopmentModels.swift`, `ClinicalMilestoneCatalog.swift`, `OnboardingFlowView.swift`, `NotDiagnosticFooter.swift`, `apps/ios_asd_gaze_app/README.md` |
| WP-S.3 | impl | Store listing rewrite in all locales, beta changelog, Snapfile | `apps/ios_asd_gaze_app/fastlane/metadata/**`, `fastlane/Fastfile` (changelog string only), `fastlane/Snapfile` |
| WP-S.5 | senior | Consent flow, permission primers, per-feature opt-ins, Privacy & Your Data screen, pre-session disclaimer screen, re-consent gate, tests | new `Sources/Models/ConsentDocument.swift`, `ConsentRecordStore.swift`, `FeatureOptIns.swift`, `PermissionPrimer.swift`; new `Sources/Views/Consent/*.swift` (`ConsentFlowView`, `ConsentReviewView`, `PermissionPrimerView`, `PrivacyDashboardView`, `BeforeYouBeginView`); `Sources/Views/OnboardingFlowView.swift`, `Sources/Models/CaptureEntryGate.swift`, `Sources/Views/GazeScreeningWizardView.swift` (wiring, after WP-S.1 lands); `Tests/ConsentFlowTests.swift`; wiring into `AsdGazeApp.swift` and `SettingsView.swift` only after WP-S.1 is integrated |
| WP-S.4 | impl | Entitlements and plist hygiene: declared-age-range entitlement, drop unused HealthKit background delivery, widget version keys | `Resources/AsdGazeCollector.entitlements`, `Widget/Info.plist` |
| WP-S.7 | impl | App Store screenshots from the store UI: `StoreSim` configuration, StoreSim-only screenshot fixture, Maestro store flows mirrored by Gherkin, runner, real captures replace the Sept 15 composites; `snapshot` (Snapfile, UITests helper) removed | `project.yml` (StoreSim), new `Sources/Models/StoreScreenshotFixture.swift`, fixture hooks in `AsdGazeApp.swift`, `CaptureEntryGate.swift`, `GazeSessionHistoryStore.swift`; Maestro ids in `OnboardingFlowView.swift`, `GazeSessionResultView.swift`, `StoreSessionsView.swift`, container ids in `BeforeYouBeginView.swift`, `PermissionPrimerView.swift`, `PrivacyDashboardView.swift`; `CameraAccessCoordinator.swift` (fixture camera); store post-session title "Observation saved" (`store_result_title` in `AppLocalizer.swift`, was "Observation Evidence Report"); `SettingsView.swift` (TrueDepth row uses the gate); new `maestro/flows/store/*.yaml`, `Tests/bdd/features/store_*.feature`, `scripts/run_store_screenshots.sh`; `fastlane/config.yml`, `fastlane/screenshots/**` (git-ignored); `scripts/check_release_symbols.sh` |

Gates (main session, after integration): `scripts/ios_test.sh` (kit tests, app tests, Release and
DemoRelease builds, claims, privacy API, release symbols), `python3 scripts/check_asd_claims.py
--swift --prose --secrets`, `ruby -c fastlane/Fastfile`, `fastlane validate_metadata`.

Still human (unchanged from `HANDOFF.md`): org Apple account and bundle id (H-6), live privacy and
support pages, stimulus clip licence (H-8), real-device checklist (U7), store screenshots from the
store build, native hi/ne review, age rating questionnaire in App Store Connect.

## Screenshots (WP-S.7, 2026-11-09)

App Store screenshots are real captures of the store UI, produced by the Maestro store flows,
so the same flows double as behaviour tests. `snapshot` is no longer used (Snapfile and the
unreferenced UITests helper are removed); `scripts/generate_app_store_screenshots.py` is not
used for EarlyGaze.

- Build: `StoreSim` configuration (debug type, simulator only, never archived) with
  `ASD_STORE_BUILD ASD_STORE_SCREENSHOTS ASD_APP_STORE_AGE_ASSURANCE`. It is not
  `ASD_SIMULATION_ENABLED`: SwiftPM builds the kit for a custom configuration without its
  `.debug` conditions, so the kit has no simulation API there (as in Release).
- Fixture (`Sources/Models/StoreScreenshotFixture.swift`, StoreSim only): the launch argument
  `-ASDStoreScreenshotFixture YES` lets a simulator stand in for a TrueDepth device on first
  run (capture entry, the Settings sensor row, and a camera iOS has not asked about yet, so the
  in-app primer shows; a simulator otherwise grants the camera silently) and shows three example sessions in memory (marked
  simulated, no score values, never persisted); `-ASDStoreScreenshotScreen result` also opens
  the post-session screen. No camera frame, stimulus clip (H-8) or simulation badge appears.
  `scripts/check_release_symbols.sh` asserts the launch keys are absent from Release and
  DemoRelease.
- Flows: `apps/ios_asd_gaze_app/maestro/flows/store/01-consent-and-onboarding`,
  `02-capture-entry`, `03-session-and-sessions`, `04-privacy-and-settings` (ids only, so they
  run in every language), specified by `Tests/bdd/features/store_*.feature`.
- Screens: `01_consent_overview`, `02_consent_sign` (guardian "A. Parent"),
  `03_before_you_begin`, `04_camera_primer`, `05_recording_quality`, `06_sessions`,
  `07_privacy_your_data`, `08_settings_about`.
- Runner: `apps/ios_asd_gaze_app/scripts/run_store_screenshots.sh [--device NAME] [--lang
  en-US|hi|ne-NP] [--skip-build]`; defaults to iPhone 18 Pro Max (6.9-inch, 1320x2868) and
  iPad Pro 13-inch (M5) (2064x2752) in en-US only (S-14); `--lang hi --lang ne-NP` still works
  for the parked locales. Output goes to
  `fastlane/screenshots/<lang>/<Device Name>-NN_name.png` (git-ignored, regenerate before
  `fastlane upload_screenshots`); Maestro logs stay in `maestro/screenshots/<device>/<lang>/`.
- The consent, onboarding and most store copy are English in every locale today (WP-S.6), so
  hi and ne-NP screenshots would show English text apart from the localised strings; none are
  kept while the listing is English-only (S-14).

## Status (2026-11-08, end of day)

Landed on `dev`: WP-S.1, S.2, S.3, S.4, S.5. Gates on the final tree: Release and DemoRelease
build; app tests 467 passed, 1 skipped; claims (`--swift --prose --secrets`), privacy API use,
release symbols (Release and DemoRelease), UI literal lint (0 new, 13 stale removed),
`fastlane validate_metadata`, `ruby -c Fastfile`, asd-cli i18n tests. Kit tests: 258 of 269
pass in the sandboxed session; the 11 failures are temp-file writes blocked by the sandbox
(kit source unchanged); run `swift test --package-path domains/asd_screening/ios` unsandboxed.

Open before submission (code): WP-S.5b opt-in gating; WP-S.6 shared consent text; screenshots
from the store build (done in WP-S.7, see Screenshots); README/HANDOFF drift
sweep. Open before submission (human): H-6 account, bundle id and live privacy/support pages
(the in-app links use the same placeholder URLs as the listing); enable the Declared Age Range
capability on the App ID; H-8 stimulus clip licence (clips are gitignored and absent from the
committed project, so a clean checkout archives with no clips); U7 real-device checklist (file
protection, consent flow, camera primer, capture); native hi/ne review; App Store Connect age
rating questionnaire; App Review notes explaining the clinic-code enrolment and that no result
is shown on the device.
