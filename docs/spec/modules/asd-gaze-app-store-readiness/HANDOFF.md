# Handoff: ASD Gaze App Store readiness (as of 2026-10-24)

Read first: `work-packages.md` (authoritative for file ownership and traps), `technical.md`, `acceptance.md`,
`qa-validation.md`, ADR-0001..0005. Working rules: spec before code; test-first; implementers do not commit;
the main session runs every gate and commits what passed; builds only under `/private/tmp/...`.
Model routing: architect (Fable) designs, `senior-implementer` (Opus) for subtle packages, `implementer`
(Sonnet) for mechanical ones.

## State

Branch `dev`, pushed to `origin` at `9e025c3`. Not pushed to the `cursor` remote.
Phase 1 (blockers) is DONE: WP-1.1, 1.2, 1.3, 1.4, 1.5a, 1.5b, 1.6. PR #2 (gaze reaction scoring) is merged
(reconciled with the nil/optional metric contract).

Gates at handoff (all commands in `qa-validation.md`):
- Kit: `swift test --scratch-path /private/tmp/asd-kit` -> 150 tests, 0 failures (one earlier run showed 18
  failures that did not reproduce in 6 reruns; watch for flakiness).
- App: 162 tests passing (iPhone 18 Pro simulator), Release build succeeds.
- `python3 scripts/check_asd_claims.py [--swift|--secrets|--selftest]` all pass; CI `asd-claims` job runs `--swift`.
- Repo CI on `dev` was already red before this work (C++ build, mobile strings/shapes, coverage, sanitizers).
  Not investigated.

## Decisions made (H-items)

- H8: working tree committed in chunks.  H1: secrets removed going forward only, NO history rewrite
  (`git filter-repo` command documented in `apps/ios_asd_gaze_app/fastlane/README.md`, not run).
- H2: name "EarlyGaze Observation Diary" (placeholder). H7: no research study at launch; study/IRB framing removed.
- Category HEALTH_AND_FITNESS (H6 not formally confirmed).

## Still needs the human

1. Rotate the committed password (still in history on `origin` and `cursor`). Decide on a history rewrite + force push.
2. H3 M-CHAT-R/F licence; H4 org Apple developer account, bundle id (currently `org.curv.asd.gazecollector`),
   domains. `curv.engineering` is a placeholder publisher domain (age-suitability URL etc.): confirm.
3. Decode default: a pre-change `GazeSessionReportSummary` with no `captureSource` decodes as `.trueDepth`
   (kit). Alternative is to throw. Pre-v1.6 records without the no-video guarantee decode as simulated/legacy.
4. Native-speaker review of Hindi/Nepali store text (mandatory sentences are in English).
5. Manual device checklist (acceptance scenarios 1, 10-16, 19). File protection cannot be verified on the
   simulator or macOS; the vault, sync queue and logger protection must be checked on a real iPhone.

## Remaining packages

Phase 2 (high):
- WP-2.1 actor-owned gaze pipeline: adopt the existing `GazeProcessingActor` (do not rewrite), TSAN preset
  (`swift test --sanitize=thread`), timestamp-based durations (currently assume 60 fps), real
  `interruptionCount`/`interruptedSec`. `stopSession()` becomes async: `ConsentGatingTests` calls
  `_ = controller.stopSession()` and needs `await`. Fix: kit `stopSession()` on an empty session labels it
  `captureSource ?? .trueDepth`; it should not claim live data.
- WP-2.2 camera permission flow, idle timer, scene phase.
- WP-2.3 research endpoint from config (HTTPS only), privacy manifests, usage strings. `http://127.0.0.1` is an
  IP literal that bypasses ATS; app tests currently attempt real network calls to it ("Transmission failed" logs).

Phase 3 (medium): WP-3.1 FHIR Bundle/Provenance/real coding (SNOMED/OMOP codes untouched so far);
WP-3.2 diary/milestones/medications to FHIR; WP-3.3 iOS CI (`ios.yml`, `scripts/ios_test.sh`; also add
`scripts/check_release_symbols.sh` as a CI step); WP-3.4 publisher identity, M-CHAT gate, stimulus provenance
(needs H3, H4).
Phase 4 (low): WP-4.1 URLs/locales; WP-4.2 Swift 6 strict concurrency as errors; WP-4.3 protected history store
out of UserDefaults; WP-4.4 renames, last.

## Known follow-ups and traps

- `MainGazeScannerView` writes the CareKit outcome twice (second overwrites first; pre-existing).
- In Debug the classifier runs twice per session (stimulus `onComplete` no longer passes a classification).
- Kit `RiskStratification.elevatedRisk` raw value compiles in every configuration (check_release_symbols.sh
  passes today; keep an eye on it).
- Delete-all: observation data is deleted when the first dialog is confirmed; answering "Keep care plans" on the
  second prompt does not undo it.
- `ASD_ML_DATA_EXPORT` is not defined in `project.yml`; `BlendShapeLoggerTests` run only with
  `OTHER_SWIFT_FLAGS='$(inherited) -DASD_ML_DATA_EXPORT'`.
- Teardown bug in `ResearchKitSyncServiceTests`/`ResearchKitStudyTests`: `removePersistentDomain(forName:
  testUserDefaults.description)` leaves settings files behind.
- `xcodegen generate` rewrites the tracked `project.pbxproj`; it drops resources whose files are absent (e.g.
  stimulus .mp4s in a fresh worktree). Do not commit a pbxproj regenerated in such a tree.
- `xcodebuild test` occasionally hangs at launch; `test-without-building` after a build worked.
- The claims script scans only lines that call `Text(`/`Label(` etc. Clinical wording in model strings
  (`ChildProfile.clinicalFocus`, `MCHATScoreResult.clinicalRecommendation`) is not caught; consider widening it.
- Parallel agents share one tree and the scratchpad: use distinct derived-data paths and never restore from the
  scratchpad with a wildcard.
- `docs/LAUNCH_PLAN.md` (open-spectrum-tracking repo) was not found locally; its package rows were NOT updated.
  Mark WP-1.1..1.6 done there.
