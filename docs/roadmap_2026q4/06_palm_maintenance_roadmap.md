# 06 — Palm Hand-Scan and Vastu: Freeze Plan

> Decision 2026-10-27: palm and vastu are in **maintenance mode**. No feature work until ASD Phase 3 exits. This doc exists so agents do not "helpfully" continue the old palm roadmaps.

## 1. State

- `domains/palm/` (~3.7k LOC C++, 11 tests): landmarks → 512 px canonical warp → Steger ridges → crease linking → Heart/Head/Life/Fate analysers, mounts, marks → `domain_payload`. `VlmPromptFormatter` builds prompts; no live LLM call.
- `apps/ios_palm_app` (~9.1k Swift, 38 unit tests + 1 UI smoke; README says xcframework "not linked yet" while `build/CurvEngine.xcframework` exists — verify), `apps/android_palm_app` (~3.6k Kotlin, no tests, creases are landmark-guess fakes per its README).
- `apps/*_vastu_app`: thin prototypes (~240 Swift / ~360 Kotlin), no tests, no project files for iOS.
- Store readiness (`docs/palm_app_store_readiness.md`, 2026-09-17): **not ready** — privacy/support URLs point at an unrelated astrology page, unsigned archive, no device testing, Guideline 4.3(b) risk.
- Pending old roadmaps: `palm_robust_roadmap.md` D1–D26 all unchecked (CV→ML verifier, no model); `palm_prediction_roadmap.md` Track 7–8 not started; thresholds `calibrated:false`; 52 samples, review status unaudited.
- Known science caveats (PROGRESS): hysteresis thresholds inert (shared with flood, F10); CLAHE kills micro-signs; dark-line extraction blind on canonical capture; main pipeline junction list empty so forks/grills read nothing.

## 2. Freeze rules

1. No new palm features, no store submission work, no ML verifier work.
2. Keep palm tests green because they are the regression net for `core/` changes (notably WP-X.3 hysteresis normalisation). If a core change breaks a palm test, fix the test expectation only with a documented reason.
3. Hygiene only: U2 (demo credentials in fastlane review info), U3 (personal hand photos), stale README claims.

## 3. Minimal work packages

| WP | Title | Tier | When |
|---|---|---|---|
| WP-P.1 | Remove credentials/phone/email from `apps/ios_palm_app/fastlane/metadata/review_information/`; `.env.example` pattern; rotate password | I | this week (U2) |
| WP-P.2 | Untrack `data/samples/mobile_captures/*bishal*`; fix `.gitignore` glob; add a small synthetic palm fixture if tests need one | I | this week (U3) |
| WP-P.3 | README truth pass: xcframework linked or not; Android fake creases banner; "maintenance mode since 2026-10-27" banner on both apps and `domains/palm/README.md` | I | this week (U10) |
| WP-P.4 | Move `docs/palm_*`, `docs/handover_palm_*`, `docs/research/palm_next_phase.md` into `docs/palm/` with an index; mark roadmaps *paused* | I | Nov 2026 |
| WP-P.5 | (Only if palm resumes) Calibration flywheel: audit the 52 labels, fix junction list, recalibrate after WP-X.3 | S | not scheduled |

## 4. If palm resumes later

Start from `docs/research/palm_next_phase.md` (do not rebuild the detector; wire the linker, emit structured reports) and `docs/product_vision.md` (edit-to-learn loop). The admin console pattern from `03` (review queue, labels, config versions) is reusable for palm's human-validation loop, which is what `docs/admin_dashboard.md` was approximating statically.
