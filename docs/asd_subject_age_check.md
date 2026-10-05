# Subject Age Check — On-Device Assistive Age-Band Estimation

*Shipped 2026-10-05 (W5c). Companion docs: `domains/asd_screening/docs/ML_ROADMAP.md` §3,
`tools/asd_age_model/README.md`, `models/asd_age/README.md`.*

## What this feature is (and is not)

**Is:** an on-device classifier that watches the ARFace stream already used for gaze
tracking and estimates a coarse age band for the face in front of the camera. It powers
three assistive surfaces:

1. **Onboarding quick check** (Step 2) — after the caregiver picks an age bracket, an
   optional ~5 s camera check softly confirms the estimated band matches the pick
   (e.g. catches "parent filled the profile and stayed in frame").
2. **Positioning hint** — during wizard positioning, a stable teen/adult reading shows
   *"It looks like an adult is in front of the camera — bring your child into the oval."*
3. **Session metadata** — `GazeSessionReportSummary.subjectAgeSignal` (band + agreement +
   confidence floats) rides along in the vault, feeding age-normalized scoring later.

**Is not:** age *verification*. Exact age estimation of 1–5 year olds is not achievable
with public models — they systematically overestimate young children (public datasets
scarcely cover under-5 faces), with errors larger than the app's 12/18/24/36/48-month
bracket boundaries. Telling an adult face from a toddler face, however, is highly
reliable — that is what ships. Estimates never block a flow; every prompt is dismissible.

## Architecture

```
ARKit ARFrame (60 Hz, existing ARSession — no new camera pipeline)
  └─ GazeSessionController.session(_:didUpdate:) [arDelegateQueue]
       ├─ ageFrameMetrics: project ARFaceAnchor → image-space center/radius + pose/blink
       └─ SubjectAgeEstimator.record (synchronous, buffer consumed in-place, never stored)
            ├─ quality gate: pose (±35°/±30°), 30–75 cm, eyes open  [GazeQualityGate-aligned]
            ├─ throttle ≤5 Hz
            ├─ FaceCropExtractor: crop → rotate upright → 224×224 BGRA (CoreImage)
            └─ AgeBandEstimationEngine (CoreML, ANE) → FacialAgeBand + confidence
                 └─ 2.5 s rolling window → SubjectAgeSignal (modal band, agreement, isStable)
                      └─ @Published on main → UI prompts / session summary
```

Bands: `toddler` (12–36 mo) · `preschool` (37–72 mo) · `child` (7–12 yr) ·
`teenAdult` (13+ yr). Window ties break toward the younger band so an ambiguous
adult/toddler mix never triggers an "adult detected" prompt.

## Graceful degradation

No model installed, non-TrueDepth device, or simulator → the engine is inert
(`isModelAvailable == false`), no crops are rendered, `subjectAgeSignal` stays nil,
and the UI hides its affordances. No behavior changes for existing users.

## Model

MobileNetV3-Small, 4-band classifier, trained by `tools/asd_age_model/train.py`
(UTKFace bands, class-balanced, ImageNet normalization baked into the CoreML graph).
`validate.py` gates shipping on ≥0.90 recall for toddler AND teenAdult. Convert with
`convert_coreml.py` → `AgeBand.mlpackage` → `coremlcompiler compile` → `AgeBand.mlmodelc`.
`AgeBandEstimationEngine.discoverCompiledModelURL()` finds it in the app bundle or in
`Application Support/CurvAsdGazeKit/Models/` (sideload, no app update).

Future accuracy work: fine-tune on the app's own consented norm-collection data
(guardian consented, band-labeled) — see `tools/asd_age_model/README.md`.

## Privacy posture

- Fully on-device; zero network; no image, crop, or embedding ever persisted.
- Session records carry only `subjectAgeSignal` (band string + confidence floats).
- No new permission strings; the camera use is the existing TrueDepth gaze capture.
- App Store age *compliance* does not use biometrics at all — it uses Apple's
  Declared Age Range API (below).

## App Store vs internal builds

`project.yml` compiles `Release` with `-DASD_APP_STORE_AGE_ASSURANCE`:

- **Release (App Store/TestFlight):** onboarding completion additionally requests the
  parent-declared age range via `DeclaredAgeRangeService` (`AgeRangeService.shared`,
  iOS 26+, age gates 13/16/18). Result is advisory.
- **Debug/internal:** the flag is absent → `AgeAssuranceGate.isAppStoreAgeAssuranceBuild`
  is false → the API is never touched; the app behaves exactly as before.

## Test coverage

- Kit: 20 tests in `AgeBandEstimationTests.swift` (band mapping, engine inertness,
  crop math/rotation/dimensions, windowing, throttling, quality gates, tie-break,
  drain behavior) — green on macOS and iOS simulator (103 kit tests total).
- App: 9 tests in `SubjectAgeCheckTests.swift` (verdicts, hint gating, status line,
  controller state, dismissal, internal-build bypass).
