# ASD Gaze App, App Store Readiness - Acceptance Criteria

Each scenario names the technical-spec identifiers it proves. "Release build" means
the `Release` configuration from `project.yml` (no `ASD_SIMULATION_ENABLED`,
`ASD_ML_DATA_EXPORT` or `ASD_RESEARCH_CLASSIFIER` flags).

## User scenarios

### Simulation and provenance (SIM, PLC)

1. A parent installs the Release build on an iPhone whose front camera does not support ARKit face tracking. Opening the observation tab shows "This device cannot run gaze observations" with the supported-device list; there is no way to start a session and no menu offers simulation. (SIM-2, SIM-6, SIM-7)
2. A developer runs a Debug build in the iOS Simulator, starts a simulated session, and completes it. The result screen shows a "Simulated data" badge; the session does not appear in the caregiver history, is not counted in CareKit, cannot be exported to FHIR (the export button is disabled with the reason), and the vault file name ends in `.sim.jsonl.enc`. (SIM-3, SIM-5, SIM-8)
3. A parent runs a real session in which the child never looked at the eye or mouth regions. The result screen shows "Not observed" for eye and mouth dwell; the FHIR Bundle contains no component for those metrics and none for first-look latency. (PLC-2, PLC-3)
4. A reviewer reads a FHIR Bundle exported from the Release build and can find, for every Observation, a Provenance that names the app as the recording device and the capture source as TrueDepth. (SIM-4, FHIR-1)
5. A session whose ARKit frames arrived at 30 fps (older device, thermal throttling) reports a duration within 5 % of the wall-clock duration shown on screen. (PLC-5)

### Claims and naming (CLM)

6. App Store reviewers see a name, subtitle, keywords, description and release notes that describe an observation diary and contain none of the denylisted terms. The description's disclaimer paragraph says the app is not a diagnostic tool and is part of Open Spectrum Tracking. (CLM-1, CLM-5, CLM-6)
7. After a session, a parent sees the attention flag copy ("This is a screening signal, not a diagnosis") and no risk percentage, no "Elevated Risk" badge, and no "confidence". (CLM-3)
8. A parent completes the M-CHAT-R/F questionnaire and sees "M-CHAT-R/F: medium (follow-up interview needed; discuss with your pediatrician)" rather than a bare "Medium Risk" chip. The PDF uses the same wording. (CLM-4)
9. A contributor adds a string "clinical-grade accuracy" to a SwiftUI `Text`; CI fails with the offending file and line. (CLM-2, CI-1)

### Consent, retention and camera (CNS, CAM)

10. A fresh install: the camera permission prompt never appears before the parent has accepted the privacy and consent step of onboarding. (CNS-1, CAM-1)
11. A parent denies camera access. The observation tab shows an explanation and an "Open Settings" button; nothing crashes; no session is recorded. (CAM-1)
12. During a session the phone receives a call. On return, the app shows that the observation was interrupted, the summary carries the interruption, and the quality verdict is at best "degraded". The screen never dims during a session. (CAM-2, CAM-3)
13. The parent backgrounds the app mid-session. The session stops, the partial data is kept and labelled, and the idle timer is restored. (CAM-3)
14. In Settings the parent taps "Delete all observation data", confirms, and afterwards: the vault directory is empty, the history is empty, no sync queue files exist, calibration is reset, enrollment is cleared. CareKit care plans are untouched unless the second prompt was accepted. (CNS-6)
15. A parent revokes research consent while three payloads are queued offline. The queue, dead-letter and quarantine folders are empty afterwards; reconnecting to Wi-Fi uploads nothing. (CNS-5)
16. Inspecting the app container of a Release build after a session with a file-system tool shows no `BlendShapeML` directory and no `.csv` files; all vault and queue files carry complete file protection and are excluded from backup. (CNS-2, CNS-3, CNS-4)
17. The child's name entered in onboarding does not appear in any file under the app container except the profile JSON, nor in any exported FHIR or ResearchKit JSON. (CNS-7)
18. The Release build never shows research enrollment, consent document or sync UI unless `ASDResearchStudy` is configured in Info.plist with an https ingest URL. (CNS-9)
19. The camera usage prompt text describes gaze and facial-expression processing on device and, when the age-check feature is compiled in, the optional age-range check. (CNS-8, PRV-3)

### Concurrency (CONC)

20. Running the kit tests under Thread Sanitizer reports no races in a 10-second synthetic session with calibration updates, AOI battery switches and clip-clock updates arriving from another thread at 30 Hz. (CONC-1, CONC-3)
21. Frames delivered up to the instant `stopSession` is awaited are included; frames delivered after are dropped. Tested deterministically with the actor. (CONC-4)

### FHIR (FHIR)

22. The exported Bundle validates structurally: every reference inside the bundle resolves to a `fullUrl`, every Observation has `status`, `code`, `subject`, `effectiveDateTime`; the M-CHAT QuestionnaireResponse references `http://loinc.org/q/80628-1` and has 20 items with LOINC linkIds. (FHIR-1, FHIR-4, FHIR-7)
23. A mood diary entry, a milestone and a medication log exported together produce one Observation each for the first two and one MedicationStatement for the third, all with the same Patient subject. (FHIR-5)
24. No exported Observation component uses SNOMED `247540003` as its code. (FHIR-3)

### CI and configuration (CI)

25. A pull request touching `domains/asd_screening/ios/**` runs the kit tests, generates the Xcode project, runs the app unit tests on a simulator and performs an unsigned Release build; derived data lives under the runner temp directory. (CI-1, CI-2)
26. `git grep` for the previously committed phone number, personal email addresses and demo password returns nothing in the working tree; `fastlane/metadata/review_information/` is ignored. (CI-4)
27. `age_suitability_url.txt` and the other URLs resolve to the publishing organisation's domain. (CI-5)

## Success criteria

- All scenarios above pass on an iPhone with TrueDepth (iOS 18+) for the device
  scenarios and on the iOS Simulator for the rest.
- `python3 scripts/check_asd_claims.py` exits 0 on the repository.
- Kit tests pass with and without `-sanitize=thread`; app tests pass on the simulator.
- The human-decision items in `non-technical.md` are closed before submission.

## Edge cases

- Legacy v1.5 records in history decode with `legacyProvenance: true` and export with
  `Provenance.activity` text "legacy record, capture source assumed TrueDepth".
- Keychain unavailable at first launch: the vault uses an ephemeral key as today and
  the UI warns that sessions will not be readable after relaunch (existing behaviour,
  kept; `SessionVaultManager.isKeyPersisted`).
- `ResearchStudyConfiguration` present but URL is `http://`: the research section is
  hidden and a log line explains why; nothing is uploaded.
- The quality gate rejects a session (`validRatio < 0.5`): no Observation is produced;
  the history still records the attempt with verdict `rejected` so parents see the
  attempt and can retry.
- Simulator run of the Release configuration (possible locally): behaves as scenario 1.
