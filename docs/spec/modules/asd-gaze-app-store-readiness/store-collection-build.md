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
| S-7 | Working name "EarlyGaze" (display name and store name); description calls it "EarlyGaze Observation Capture". Placeholder until H-9. | The tree had three names ("Eye Gaze", "EarlyGaze Observation Diary", "ASD Gaze"). "ASD" never appears in user-facing text. | Amendment 2026-11-09: the App Store Connect record (app id 6828468388, bundle id `com.open-spectrum.eye-gaze`) is currently named "Eye Gaze"; the listing rename to "EarlyGaze" is a metadata change on the next version, confirmed by the owner 2026-11-09 (name availability is checked by `deliver`). The widget App ID `com.open-spectrum.eye-gaze.carewidget` exists in the Developer portal, so the embedded care widget needs no change for the store build.
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

## Opt-in gating (WP-S.5b, design 2026-11-09)

Findings behind the package (code survey 2026-11-09):

- The six toggles in Privacy & Your Data (S-12) persist a choice, but only `ageRangeCheck` is
  read by its feature. The row copy admits it ("Controls whether this feature is offered").
- In the store build, reminders, HealthKit mood, dictation and photo attachments have no
  surface at all: they live on the dashboard, in care alerts and in the diary, which S-2 and
  S-6 hide. A toggle for a feature the build cannot use is itself an untrue statement.
- Clinic delivery cannot work in the store build as wired: `ResearchKitSyncService` refuses
  to queue or flush unless `ResearchStudyEnrollment.isConsented`, and the only screen that
  sets it (`ResearchStudyConsentView`) is a dashboard modal the store build hides. The
  failure text ("Parental research consent is not active.") is research wording (5.1.3).
- `NormCollectionToggleRow` (research wording) is not behind `#if !ASD_STORE_BUILD`; it would
  appear in a store build the moment a clinic configuration ships (S-4).

| # | Decision | Why |
| --- | --- | --- |
| S-15 | Every opt-in gates its feature in every build: `FeatureOptIn.isGated` is `true` for all cases, `notYetGatedNote` and the row suffix are removed. Each feature reads `FeatureOptIns` at its entry point **and** in the service behind it (defence in depth): clinic delivery in `ResearchKitSyncService` (`enqueueSession`, `requestFlush`, `syncNow`), reminders in `CareAlertScheduler` (`requestAuthorizationIfNeeded`, `resync`) and the Settings care-alert toggles, HealthKit in `HealthKitMoodStore` (`requestAuthorization`, `save`, `fetchRecentStateOfMind`; `deleteHealthSample` stays allowed so a guardian who turned the feature off can still remove what was written) and the Settings row, dictation and photos in `ObservationQuickLogBar` and `ObservationDictationController.start`. Services take the opt-in as an injected closure so tests do not touch `UserDefaults.standard`. | S-12 honesty rule; 5.1.1(ii). |
| S-16 | The store build shows only the opt-ins whose feature exists in that build: `FeatureOptIn.available(storeBuild:)` returns `[.ageRangeCheck, .clinicDelivery]` for the store build and `allCases` otherwise. `PrivacyDashboardView`, the detail view and `resetAll` iterate the available list; the stored flags for hidden features stay `false`. | A switch for an absent feature misleads the guardian (S-12). |
| S-17 | Clinic delivery is authorised when **all** hold: the `clinicDelivery` opt-in is on; the guardian consent record matches `ConsentDocument.current` (`ConsentRecordStore.shared`); in Debug/DemoRelease additionally the research-study enrolment `isConsented` (existing behaviour, unchanged). One pure type `ClinicDeliveryAuthorization` (new file `Sources/Models/ClinicDeliveryGate.swift`) computes this from injected inputs; `ResearchKitSyncService` takes `deliveryAuthorized: @MainActor () -> Bool` (default: the real gate) and consults it in `enqueueSession` (refuse, remove the stray research log, log the reason), `requestFlush` (no-op) and `syncNow` (`.failed(reason:)` with the neutral text "Clinic delivery is off." or "Consent is not active."). The string "research" never appears in store-build status text. | The store build has no research consent screen; the guardian consent (S-10) is the consent that covers delivery. |
| S-18 | Turning clinic delivery **off** stops every send (nothing new is queued, nothing queued is flushed, the retry timer and launch flush are no-ops through `requestFlush`/`syncNow`); items already in the queue stay on the device and resume when the opt-in is turned back on. Background `URLSession` tasks already in flight finish. Delete-all still purges the queue. Copy on the toggle's detail page says exactly this. | No silent data loss, no silent sending. |
| S-19 | Enrolment follows the opt-in. The store build's Settings privacy section shows `DeviceEnrolmentRow` whenever `isSessionSyncEnabled`, not behind `isResearchEnabled`, and `NormCollectionToggleRow` is wrapped in `#if !ASD_STORE_BUILD`. The row's code field and Enrol button are enabled only while `clinicDelivery` is on; off, it reads "Turn on Clinic delivery in Privacy & Your Data to enrol this device." Its literals move to `Localizer.tr` keys (no new baseline entries) and lose the "consented to share" research phrasing. `ClinicDeliveryStatusRow` in Privacy & Your Data and `ClinicSyncStatusLine` (Sessions tab, result screen) show a delivery-off state: "Clinic delivery is off. N session(s) waiting on this device." `SessionStorageState` gains `.deliveryOff` with verdict-free text (covered by `testStorageCopyHasNoVerdictWording`). | S-4; the listing promises that nothing leaves the device without the guardian's choice. |
| S-20 | "Before you begin" states delivery truthfully: `recordedItems(optIns:deliveryConfigured:isEnrolled:)` says "Delivered to your clinic" only when the opt-in is on, sync is configured and the device is enrolled; "Kept on this device until your clinic enrols it" when on but not enrolled; "Kept on this device only" otherwise. | S-13. |
| S-21 | The facial-expression toggle is **not** reintroduced in this package. Consent already lists facial-expression measurements under "What is recorded" (S-12 as amended), which is true today. Dropping blendshape metrics per session in the kit (`FacialAffectMetrics`, `smileReactivityIndex` and siblings in `GazeSessionRecord` and the ingest payload, console schema impact) is **WP-S.5c, after submission**. | Kit and schema change; not needed for an honest submission. |

Implementation notes: new strings go through `ConsentL10n.text(key, english)` in the consent files and
`Localizer.tr` elsewhere (keys added to `Localizable.xcstrings` with hi/ne drafts marked
`needs_review`); the literal baseline must not grow. Reminder opt-in changes trigger
`CarePlanStore.shared.resyncAlerts()` (non-store builds) so pending notifications are removed at
once. The StoreSim screenshot `07_privacy_your_data` changes (two toggles); regenerate with
`scripts/run_store_screenshots.sh` and update `Tests/bdd/features/store_privacy_and_settings.feature`
and `maestro/flows/store/04-privacy-and-settings` if they count toggles.

Tests: `ConsentFlowTests` (all gated, store availability list, no "Controls whether" text, denylist
sweep over the new copy), `ResearchKitSyncServiceTests` (opt-in off: no queue file and no research
log after `enqueueSession`, `requestFlush` no-op, `syncNow` fails with the neutral reason; on again:
the queued item flushes), `ClinicDeliveryAuthorization` truth table, `CareAlertScheduler` pure
decision helper, `HealthKitMoodStore` save/fetch short-circuit, `StoreBuildTests` `.deliveryOff`,
`BeforeYouBeginView.recordedItems` three states.

## Status (2026-11-09, end of day)

Landed on `dev`: WP-S.5b (S-15..S-21), App Review notes (`fastlane/review_notes.txt`, sent by
`update_metadata`, checked by `validate_metadata` and the claims script), claims script flags made
additive (the `ios_test.sh` claims gate previously ran only the secrets scan), README and HANDOFF
sweep. Gates on the final tree: see the tracker row. Open before submission (code): WP-S.7b polish.
Open before submission (human): `HANDOFF.md`, "Before submission: human".

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
