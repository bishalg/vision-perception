# ADR-0002: Consent-gated capture, on-device retention and deletion model

Status: Proposed, 2026-10-22
Spec ids: CNS-1..9, CAM-1..3, PRV-1..3

## Context

Two consent layers exist: the onboarding privacy/consent step
(`ChildProfile.hasConsentedPIA`, `OnboardingFlowView.swift:108-160`, `:810`) and the
research study enrollment (`ResearchKitStudyStore`, `ResearchStudyConsentView`).
Neither gates data capture in code:

- `BlendShapeCSVLogger.shared.startListening()` runs at launch (`AsdGazeApp.swift:53`)
  and appends 52 blend-shape coefficients per 200 ms to a plain CSV in Application
  Support (`BlendShapeCSVLogger.swift:25-31`, `:104-109`) whenever any session is
  recording. It is described as training data for Core ML. No consent, no file
  protection, no backup exclusion, 50 MB rotation.
- The AES-GCM vault (`SessionVaultManager.swift`) and the research upload queue
  (`ResearchKitSyncService.swift:93-94`, `:143`) are written without
  `FileProtectionType` and are included in iCloud/iTunes backups, so "never leaves
  the device" is not true for the ciphertext or the plaintext queue.
- The child's real name becomes the FHIR subject and the persisted `participantId`
  in the main stimulus flow (`GazeStimulusScreeningView.swift:575`,
  `GazeSessionResultView.swift:55`, `:604`).
- The consent document hardcodes a fictional IRB number, institution and principal
  investigator (`ResearchKitStudyModels.swift:124-131`); the ingest endpoint is
  `http://127.0.0.1:8089` (`ResearchKitSyncService.swift:78`).
- Revoking consent leaves queued payloads on disk (`ResearchKitStudyStore.swift:73-86`).
- There is no delete-my-data action although `ETHICS_PROTOCOL.md` promises one.
- The age estimator consumes every captured frame (`GazeSessionController.swift:616-627`)
  and the camera usage string does not mention it (`project.yml:47`).
- No handling of camera permission, `ARSession` failure or interruption; the screen
  can dim mid-session.

## Decisions

1. Capture is gated by onboarding consent in the app layer through a `ConsentToken`
   value that only `ChildProfileStore` can mint after `hasConsentedPIA`. The kit
   stays consent-agnostic (it is a library), the app cannot start an `ARSession`
   without the token.
2. The blend-shape CSV logger is not part of the product. It compiles only under
   `ASD_ML_DATA_EXPORT` (never in Release) and, even then, writes only during
   recording for a participant who opted into norm collection, with complete file
   protection and backup exclusion.
3. Everything the app writes about a child is protected at rest and excluded from
   backup: vault (`.complete`), sync queues (`.completeUntilFirstUserAuthentication`,
   because a reconnect flush may run while locked), CSV if compiled. Data loss on
   device migration is accepted; privacy-first means telemetry does not travel in
   backups. Session history stays in `UserDefaults` for this phase (metrics only,
   token subject) and moves to a protected file in Phase 4.
4. Identity: `participantId` and the FHIR subject are always a pseudonymous token.
   `childName` is display-only.
5. The research module is configuration-driven. Without an `ASDResearchStudy`
   Info.plist dictionary carrying a real study id, IRB id, institution, PI, consent
   version and an https ingest URL, the module does not exist in the UI and the
   sync service fails closed. Fictional defaults are deleted, not hidden.
6. Revoking research consent purges every queued payload and cancels uploads.
   Deleting all observation data is a first-class Settings action that empties the
   vault, history, queues, calibration, M-CHAT results and enrollment; CareKit care
   plans are deleted only on a second, separate confirmation because they are the
   parent's own notes, not telemetry.
7. Camera lifecycle is explicit: permission check before start with a Settings
   deep link on denial; failure and interruption are surfaced on the tracking
   stage and recorded on the summary; the idle timer is disabled only while
   recording; backgrounding stops the session.
8. The age estimator is opt-in per use (only the Quick Check card enables it for
   its own session) and is disclosed in the camera usage string.
9. The privacy manifest reflects what the Release build can transmit. With no
   research configuration compiled in, nothing is "collected" in Apple's sense and
   the label is "Data Not Collected".

## Alternatives rejected

- Keep the CSV logger but ask for consent in onboarding. Rejected: it is a training
  data pipeline, not a product feature; parents cannot meaningfully consent to it
  inside a diary app, and a 50 MB plaintext face-coefficient file on a child's
  device is indefensible in review.
- Encrypt the sync queue with the vault key instead of file protection. Rejected
  for this phase: the payloads are short-lived and file protection plus backup
  exclusion is sufficient and simpler; may be revisited if the research module ships.
- Hide the research UI behind a Debug flag instead of configuration. Rejected: a
  real study will need it in Release; configuration is the honest switch.
- Ask for camera permission at app launch. Rejected: Apple's guidance and the
  consent ordering both require the prompt in context, after consent.

## Consequences

- Several views change signatures (consent token); WP-1.3 and WP-2.2 own them.
- The research dashboard card and consent view disappear from the default Release;
  the screenshots and review notes must not show them.
- `GazeSessionController` gains `isSubjectAgeEstimationEnabled`; the Quick Check
  card is the only caller that sets it.
- A `DataDeletionService` becomes the single place that knows every on-device store.
