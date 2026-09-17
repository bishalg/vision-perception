# Palm Reader App Store readiness

Assessment: 2026-09-17. **Not ready for submission.**

Scope: `apps/ios_palm_app`, bundle `org.curv.palmapp`. No HANS-branded project was found; the approved work used Palm Reader as the scope. ASD/EarlyGaze is excluded.

## Verified local improvements

- Repaired the pre-existing malformed Swift parameter in `PalmAnalysisViewModel+ScanModes.swift`. That file now matches HEAD.
- Release UI hides unvalidated advanced scan modes, retaining Debug access.
- Added English/Hindi/Nepali entertainment disclosures, an in-app privacy link, and an empty reading state.
- Removed displayed age estimates and life-line health/vitality claims; development measurement/enrichment clauses are Debug-only. Remaining readings are cultural rules, not scientifically validated predictions.
- Removed analysis JSON prefix logging on decode failure.
- Revised local store descriptions to remove capture/accuracy guarantees and unsupported advanced-feature promises. No metadata was published.
- Unified generated Info.plist version values with build settings. Added a native dependency presence check.
- Added an app privacy manifest and an OpenCV privacy resource bundle using the existing upstream OpenCV manifest. The SDK-only FileTimestamp reason remains in its own bundle, not the app manifest.
- Added six hosted unit-test files (38 test cases), XcodeGen test target/scheme wiring, and corrected the readiness script to say that it checks metadata only.

## Verification evidence

Base commit: `b23a263`; release-readiness changes prepared on `chore/palm-app-store-readiness`. Version/build: **1.0.0 (1)**. The user requested committing all changes, including the pre-existing `data/benchmark_results.json` update, whose contents were not modified by this work. No push or publication was performed.

| Check | Result |
|---|---|
| arm64 simulator Debug build | Passed |
| Unsigned device Release build | Passed |
| `build-for-testing` | Passed |
| Hosted unit tests | **38 executed, 0 failures**, iPhone 18 Pro simulator / iOS 27.0 |
| UI end-to-end smoke | **1 executed, 0 failures**: scanner launch, disclosures/privacy-link accessibility, left/right hand selection, and background/foreground recovery |
| Final full scheme gate | `TEST SUCCEEDED`; Swift compilation/type checking and both test targets passed |
| Final unsigned Release archive | Passed; not distribution-signed, exported, uploaded, or Apple-validated |
| Archive privacy resources | App `PrivacyInfo.xcprivacy` and `OpenCVPrivacy.bundle/PrivacyInfo.xcprivacy` both present |
| Archive version | 1.0.0 / 1 |
| Static framework embedding | No embedded Frameworks in inspected archive |
| Readiness shell script | `bash -n` passed; Fastlane metadata lanes not executed in this pass |
| Whitespace validation | `git diff --check` passed |
| App Store Connect read-only lookup | Blocked: CLI reported missing authentication |

Local evidence (temporary paths, not durable CI artifacts):
- `/tmp/palm-unit-final.log` — `TEST SUCCEEDED`, 38 tests, zero failures.
- `/tmp/palm-build-for-testing.log` — `TEST BUILD SUCCEEDED`.
- `/tmp/palm-final-archive.log` — `ARCHIVE SUCCEEDED`.
- `/tmp/palm-readiness-unsigned.xcarchive` — local unsigned archive.

The initial generic simulator build failed because the provided native libraries lack x86_64 simulator slices. arm64 builds pass. A listed iOS 26.5 simulator failed to launch; testing succeeded after booting the existing iOS 27 simulator and disabling parallel testing. Initial test authoring contained type/fixture mistakes, corrected before the passing run; those failures were not evidence of app defects.

Xcode used: 27.0, build 27A266a, from the locally selected Release Candidate installation. Confirm Apple's accepted upload toolchain requirements before distribution. Warnings remain, including a non-Sendable CVPixelBuffer in shared code, an unassigned icon asset, and unused variables. Swift language mode is 5.0.

## Concrete release blockers

1. **Public privacy and support content.** Fetches of `https://astro-fusion.com/privacy` and `/support` returned the AstroFusion astrology landing page rather than substantive PalmReader policies/support. WebFetch is not a rendered-browser verification, but it did not establish valid policy/support content. The app link uses the existing URL; adding a link does not repair the website. Publish and verify app-specific content before release. Explain local processing and optional user-directed biometrics JSON sharing; do not promise that shared data never leaves the device.
2. **Apple authentication/status.** `asc apps list --bundle-id org.curv.palmapp --output json` returned missing authentication. No Apple app/version/build ID or review state was established. Configure authorized credentials through the normal secure flow; do not place keys in the repository.
3. **Signing and distribution.** The archive is unsigned. Provisioning, export compliance, accepted SDK, a signed IPA, Apple validation, agreements, pricing/availability, age rating, content rights, and published App Privacy answers remain unverified. Local manifests do not complete App Privacy or encryption declarations.
4. **Physical-device and release UX acceptance.** No physical camera/depth testing, permission recovery matrix, skin-tone/lighting robustness assessment, editor-to-reading end-to-end validation, Dynamic Type/VoiceOver review, or full localization acceptance was performed. Unit tests do not replace these checks.
5. **Actual store screenshots and full capture E2E.** Existing synthetic screenshots are not accepted as verified release assets. `ScannerSmokeTests.swift` now passes against the actual app and retains a screenshot attachment in the test result. It does not simulate a successful palm capture or cover capture → edit → reading. No store screenshot set was captured or visually accepted in this pass.
6. **CI and native artifact reproducibility.** No new iOS CI workflow was delivered. The build checks dependency presence, not provenance or reproducible construction. OpenCV resources are preserved for this app; the general native repack script was not redesigned.
7. **Product review risk.** Review the current fortune-telling/saturated-category policy (Guideline 4.3(b)) and demonstrate a distinctive, high-quality scanning/editor experience. Cultural disclaimers do not guarantee approval.

## Remaining authorized local work versus external gates

Local follow-on work: extend the passing scanner UI smoke test through capture/edit/reading, finish visual/accessibility QA, device acceptance, real screenshot generation, and CI integration. These broader approved-plan items remain incomplete.

Final commit gate log: `/tmp/palm-final-unit-ui.log` records 38 passing unit tests and one passing UI test, followed by `TEST SUCCEEDED`. Expected decoder log messages come from negative-input tests and are not compiler errors or unexpected runtime failures.

Owner/external gates: working policy/support hosting, authorized ASC access, signing identity/provisioning, physical devices/testers, and confirmation before uploads, metadata publication, TestFlight distribution, submission, or release. No such external changes were made.

Release sign-off requires all gates above, a valid processed build, TestFlight evidence, and verified Apple-side readiness. This document is a status report, not a submission certification.
