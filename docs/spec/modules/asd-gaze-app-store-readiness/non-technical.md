# ASD Gaze App, App Store Readiness - Non-Technical Requirements

## Mission alignment

The app ships under Open Spectrum Tracking: parents' daily observations of autistic
children become clinically structured HL7 FHIR records; privacy-first; explicitly not
a diagnostic tool. The gaze and facial-expression engagement metrics are one more
kind of parent observation, recorded alongside the diary, milestones and medication
log, and shared with a clinician only when the parent chooses to.

## Privacy

- Camera frames are processed in memory and never stored; this is already true in
  code and becomes verifiable (no CSV logger in Release, no backup of telemetry, file
  protection on everything that touches disk).
- The child's name never leaves the profile screen and the PDF cover.
- Nothing is transmitted unless a configured research study exists and the parent has
  enrolled; the default Release build has no network upload path at all.
- Parents can delete everything in two taps.
- The App Store privacy label for the default build is "Data Not Collected".

## Licensing

- M-CHAT-R/F (Robins, Fein and Barton) is copyrighted. It is free for clinical,
  research and educational use, but inclusion in a distributed app, and any paid or
  commercial use, requires written permission from the authors. Until permission is
  on file, the questionnaire module is compiled only when `ASD_MCHAT_LICENSED` is
  set, and the Release build hides it. Human decision H3 below.
- Stimulus videos in `Resources/Videos` need documented provenance and licence; the
  implementer of WP-3.4 lists them in `NOTICE.md` or removes them.
- CareKit is BSD-3; no change.

## Clinical disclaimer

- Every result surface says "This is not a diagnostic tool." and names the mission.
- No risk percentage, probability, "elevated risk" or "specialist evaluation
  recommended" copy is shown to parents; the only interpretation is the attention
  flag with its existing cautious wording.
- The M-CHAT-R/F categories keep their official names, always prefixed with the
  instrument name and followed by the official follow-up guidance.
- The consent document is not a research consent unless a real study (IRB, PI,
  institution) is configured; fictional values are removed.

## Accessibility

- New views (`UnsupportedDeviceView`, camera-denied view, deletion dialog, simulated
  badge) use Dynamic Type, VoiceOver labels and non-colour status cues.
- The "Simulated data" badge has an accessibility label, not only a colour.

## Decisions that need a human

| Id | Decision | Why it blocks | Default if nobody decides |
| --- | --- | --- | --- |
| H1 | Rewrite git history to remove `fastlane/metadata/review_information/` (phone, personal emails, demo password) from `origin/dev` and `cursor/dev`, or accept that they stay in history and only rotate the demo password. | Pushed history; rewriting breaks every clone and the `cursor` remote. | Remove from the working tree, rotate the demo password, do not rewrite. |
| H2 | Product name. Must not contain "Test", "Autism", "ADHD", "Screening" (ADR-0004). | Store name, bundle display name, PDF header, copy. | "EarlyGaze Observation Diary" until renamed. |
| H3 | M-CHAT-R/F licence: obtain written permission or ship without the questionnaire. | Release hides the module until decided. | Hidden. |
| H4 | Publishing entity and Apple Developer Program account (Organization with D-U-N-S), bundle id prefix, support/privacy/marketing domains (ADR-0005). | Code signing, metadata URLs, FHIR namespace. | Keep `org.curv.*`, env-driven, URLs left blank so the claims script fails until filled. |
| H5 | Primary category: Health & Fitness (recommended) or Medical. | Review scrutiny differs. | Health & Fitness. |
| H6 | Whether a research study exists at launch (real IRB, PI, institution, https ingest). | Determines whether the research module ships at all. | Not configured; module hidden. |
| H7 | Whether to keep the Debug-only CoreML "classifier" code in the repo. | Maintenance burden and reviewer perception. | Keep, Debug-only. |

## Contributor responsibilities

- A spec change precedes code; the work package names the spec ids it closes.
- The main session reviews each package report against `qa-validation.md`, runs the
  full gates after integration and commits only what passed.
- ADR status is changed to Accepted by a human maintainer, not by an agent.
