# 00 — Repo State (2026-10-27) and Urgent Actions

> Source: three read-only scans run 2026-10-27 (ASD on Sonnet; flood/core and palm/hygiene on Haiku) plus spot reads by the architect. Nothing was built or executed; test counts quoted come from logs and docs, not fresh runs.

## 1. What the monorepo is

A C++20/OpenCV "perception core" (`core/`, ~29 public headers: Steger sub-pixel ridge extraction, Hessian math, quality gates, warps, pattern detection, DTW, I-VT gaze classifier) with **domain packs** (`domains/`), **adapters** (`adapters/`: C ABI, OpenCV overlay, OpenCV-DNN landmarks, CCTV, Apple, Android; ONNX/TFLite/MediaPipe are empty placeholders), and **apps** (`apps/`).

Three real products live here:

| Product | Where | State in one line |
|---|---|---|
| **CurvFlood** (OpenCV AI 2026 entry) | `domains/geospatial`, `scripts/*flood*`, `aws_infra/`, `apps/cesium_viewer`, `docs/index.html` | Submitted. Python/cv2 analytics + CDK Bedrock agent. Several real defects (see `05`). |
| **ASD gaze app** ("EarlyGaze Observation Diary") | `apps/ios_asd_gaze_app` (~23.7k Swift), `domains/asd_screening/ios` (CurvAsdGazeKit, ~5.9k Swift), `tools/asd_research_server.py` | Mature on-device app; **no real backend**; C++ pack not linked to iOS. |
| **Palm hand-scan** (+ Vastu prototypes) | `domains/palm`, `apps/ios_palm_app`, `apps/android_palm_app`, `apps/*_vastu_app` | Works locally; not store-ready; deprioritised. |

OpenCV usage: ~106 `cv::` call sites in core (`sepFilter2D`, `Laplacian`, `warpPerspective`, CLAHE, `parallel_for_`), `cv::dnn` only in `adapters/opencv_dnn`, cv2+numpy in the flood Python scripts. The **iOS ASD app does not use OpenCV or the C++ core at all**; gaze comes from ARKit `ARFaceAnchor` and all scoring is Swift.

Build: CMake presets (`release`, `debug`, `asan`, `tsan`, `coverage`), GoogleTest (46 CTest registrations, last log 46/46 on 2026-10-24), pytest (≈83 Python tests), XcodeGen + `scripts/ios_test.sh` for ASD iOS (≈164 kit + ≈285 app test functions by grep), GitHub Actions `ci.yml` + `ios.yml` (coverage job is non-blocking; CI on `dev` was reported red before the last ASD WPs and not investigated).

## 2. Cross-cutting findings that change plans

1. **Nebius × NVIDIA hackathon is still open — deadline 2026-10-30 22:30 IST (3 days).** `hackathons/NEBIUS_NVIDIA_2026.md` checklist is entirely unticked. Its rules need a public Apache-2.0/MIT repo; this repo is "All Rights Reserved". `tools/nebius_nemotron_agent.py` (13 tests) exists. **Decision H-1 needed today:** skip, or spin out the planned `river-sentinel-nemotron` repo in 3 days. Recommendation: **skip** unless a 3-minute video and a separate repo can be done without touching ASD time.
2. **The repo is public and contains personal/sensitive-looking data** (none are secrets by pattern, but they are inappropriate for a public tree):
   - `apps/ios_palm_app/fastlane/metadata/review_information/` — demo login + password, a personal email and phone number (also repeated in `notes.txt`). Password is in git history on `origin`.
   - 19 tracked real hand photos / overlays of a named person under `data/samples/mobile_captures/` (two are 16–17 MB). `.gitignore` rules `data/samples/*_bishal*` don't match the subfolder.
   - `tools/asd_research_server.py` `study-info` returns a fictional IRB id and PI name; `docs/pitch_deck/*` cites "Protocol #2026-088", "FDA-grade", "21 CFR Part 11", "SOC 2 Type II" — none substantiated and contradicting ADR-0004 (non-diagnostic) and the in-app claims gate.
   - Apple team ID and Apple ID email appear in fastlane files (low risk, but tidy).
3. **ASD backend is demo-grade.** Single-file stdlib `http.server`, SQLite, no auth, `CORS *`, binds to localhost, 6 routes. The iOS `ResearchKitSyncService` only talks to an **https DNS host** (rejects IP/loopback), sends no auth or idempotency header, and is disabled unless `ASDResearchStudy` is in Info.plist. Backend weeks 2–4 of the sprint tracker were never built (items marked `[x] (deferred)` are **not built**).
4. **ASD C++ `AsdBiomarkerPack` is not linked into the iOS kit.** The pitch's "C++20 perception core" story is not how the iOS app works. Either link it (WP-D.6, low value) or stop claiming it.
5. **Pitch vs product divergence.** Code and store metadata say *observation diary, not diagnostic, no risk %, M-CHAT compiled out unless licensed*. Pitch deck says *FDA-grade screening, 16 biomarkers, 18 languages*. The demo narrative must be re-based on what Release builds show (see `01`).
6. **Flood evidence/doc mismatches** (segment counts, ridge delta, COOL benchmark run on a local Mac not Graviton, hard-coded Steger params in `run_flood_pipeline.sh`, data not reproducible from a clean clone). Judging runs 2026-10-27 → 11-09; **do not rewrite history now**, but fix for post-judging (see `05`).
7. **AWS agent has real defects**: EventBridge-shaped event → 400; synthetic positive fallback is live unless `VISION_STRICT_MODE=1` (never set in the stack); token check skippable; non-atomic idempotency; Lambda doesn't re-check thresholds. See `05 §2`.
8. **Core "domain-agnostic" invariant is violated and unenforced** (palm landmarks, palm transforms, line names, gaze types inside `core/`). See `07`.
9. **Stimulus videos have unknown provenance/licence** (`apps/ios_asd_gaze_app/NOTICE.md`): every clip is 8.0 s regardless of catalog metadata; internal-use only. Blocks TestFlight external testing and store submission.
10. **No model weights exist anywhere** (age-band model never trained; hand-landmark ONNX not vendored; `models/manifests` and `models/licenses` empty; `scripts/download_models.sh` referenced but missing).

## 3. Urgent actions — this week (2026-10-27 → 11-02)

| # | Action | Owner / agent | Effort | Status |
|---|---|---|---|---|
| U1 | Decide Nebius (H-1). If skipping, mark `hackathons/NEBIUS_NVIDIA_2026.md` as *not pursued* so agents stop planning for it. | Owner | 10 min | **Done (Skipped)** |
| U2 | Remove `review_information/` demo credentials + phone/email from tree; rotate the demo password; replace with `.gitkeep` + `.env.example` pattern. Decide H-2 whether to rewrite history (`git filter-repo`) — recommended **yes** for the password file only, after judging ends 2026-11-09 to avoid breaking judge links. | `implementer` | 1 h | todo |
| U3 | Untrack `data/samples/mobile_captures/*bishal*` (keep local), fix `.gitignore` to `data/samples/**/*_bishal*`, add a 1 MB `data/samples/README.md` policy. Replace with a synthetic or consented sample if a fixture is needed for tests. | `implementer` | 1 h | todo |
| U4 | Replace fictional IRB/PI strings in `tools/asd_research_server.py` with env-driven config (empty by default). Remove unsubstantiated regulatory claims from `docs/pitch_deck/*` or move them to an explicit "aspirational roadmap" section with "not yet true" labels. | `implementer` | 2 h | todo |
| U5 | Add `.gitignore` entries for `apps/ios_asd_gaze_app/build-sim/`, `fastlane/report.xml`, `*.profraw` already covered; confirm `git status` clean. | `implementer` | 15 min | todo |
| U6 | Pick the demo build configuration (H-3): **Release-equivalent with the traffic-light flag only** (recommended) vs Debug with research classifier. Record in `01 §3`. | Owner | 15 min | **Done (Release-equivalent flag only)** |
| U7 | Run the manual device checklist on a real TrueDepth iPhone (acceptance scenarios 1, 10–16, 19 in `docs/spec/modules/asd-gaze-app-store-readiness/acceptance.md`). Log results in `01 §6`. | Owner + `implementer` for fixes | 3 h | todo |
| U8 | Stand up backend v0 skeleton (WP-B.1) so the demo can show data landing in a dashboard. | `senior-implementer` | 2 days | todo |
| U9 | Make `ci.yml` green or explicitly waive failing jobs with a dated comment, so new ASD work has a trustworthy signal. | `implementer` | 0.5 day | todo |
| U10 | Freeze palm: add a banner to `apps/ios_palm_app/README.md` and `domains/palm/README.md` ("maintenance mode since 2026-10-27; see docs/roadmap_2026q4/06"). | `implementer` | 10 min | todo |

## 4. Human decisions register (H-n)

| ID | Decision | Resolution (2026-10-27) | Blocks |
|---|---|---|---|
| H-1 | Pursue Nebius × NVIDIA (deadline 2026-10-30)? | **No (Skip)** — team focus on ASD and core platform | U1 (Done) |
| H-2 | Rewrite git history to purge demo password + photos? | **Yes, after 2026-11-09** (after flood judging ends) | U2, U3 |
| H-3 | Demo build: Release flag-only vs Debug classifier | **Release flag-only** (`DemoRelease` configuration) | WP-A.1 |
| H-4 | Backend hosting for Phase 1–2: single Docker host (Fly.io/Railway/EC2) vs AWS-native (App Runner + RDS via CDK) | **Docker host for demo**, AWS-native from Phase 3 | WP-B.1, WP-B.7 |
| H-5 | Admin auth provider: self-hosted (FastAPI + Argon2 + JWT) vs AWS Cognito vs Clerk | **Self-hosted for Phase 1**, Cognito when moving to AWS (BAA-eligible) | WP-B.3 |
| H-6 | Organisation Apple Developer account, bundle id, real publisher domain (replaces `curv.engineering` placeholder) | Pending org setup | WP-E.2 |
| H-7 | M-CHAT-R/F licence request to instrument owners | Keep compiled out | WP-E.4 |
| H-8 | Stimulus clips: license existing, commission, or generate own | Commission/generate 8–10 clips (Default) | WP-D.1 |
| H-9 | Product name (EarlyGaze Observation Diary is a placeholder) | Keep until store submission (Default) | WP-E.1 |
| H-10 | Any real research partner / IRB? If none, keep all "study" UI config-gated and remove protocol numbers from docs | None (Default: env-driven / empty) | U4 |

## 5. Quick map of where things are

```
apps/ios_asd_gaze_app/         iOS app (XcodeGen project.yml, Sources/{App,Models,UI,Views}, Tests, maestro, fastlane)
domains/asd_screening/ios/     CurvAsdGazeKit SwiftPM (ARSessionBridge, GazeProcessingActor, PerVideo*, Inference/*, Resources/*.json)
domains/asd_screening/docs/    ARCHITECTURE, DATA_COLLECTION_SCHEMA (aspirational), ETHICS_PROTOCOL, ML_ROADMAP, NORMATIVE_BASELINES, STIMULI_SPEC
docs/spec/adr/ADR-0001..0005   Decisions incl. ADR-0004 non-diagnostic, ADR-0005 org account
docs/spec/modules/asd-gaze-app-store-readiness/   work-packages, HANDOFF, acceptance, claims-denylist
tools/asd_research_server.py   demo backend (to be replaced by services/asd_backend, see 02)
tools/asd_norm_aggregator.py   norm candidate builder (to move into backend jobs)
tools/asd_age_model/           train/validate/convert scripts (no weights)
aws_infra/                     CDK for flood agent (reusable patterns only)
scripts/ios_test.sh, check_asd_claims.py, check_release_symbols.sh, check_privacy_api_use.sh   ASD gates
```
