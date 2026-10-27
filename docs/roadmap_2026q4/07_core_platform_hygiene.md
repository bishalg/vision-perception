# 07 — Core Platform, CI and Licensing Hygiene

> Small, continuous work that keeps the other roadmaps honest. Tier mostly `implementer`.

## 1. Core "domain-agnostic" invariant — currently violated and unenforced

Palm- and gaze-specific code in `core/`: `RoiWarper.hpp` (`HandLandmarks`, `computeCanonicalPalmTransform`), `roi/CanonicalWarper.hpp` (`warpPalm`), `pattern/HostAttribution.hpp` (curve ids named heart/head/life/fate), `geometry/HandAnthropometry.cpp` (crease metrics), `preprocess/FlatFieldCLAHEPreprocessor.hpp` and `pattern/PatternValidation.hpp` (palm-width units), `temporal/GazeTypes.hpp`, `temporal/OculomotorClassifier.hpp`.

Decision: **keep the invariant but redefine the boundary honestly.**
- `core/` = geometry, ridge, quality, warps (generic quad/landmark warps), pattern primitives, temporal primitives (I-VT is generic signal processing; keep it, but rename vocabulary to "sample/fixation", no "gaze" in type names if cheap — else accept and document).
- Move palm-named things to `domains/palm/` (`HandLandmarks`, canonical palm transform, host-line names, anthropometry crease metrics). Mechanical moves with tests following.
- Enforce with `scripts/check_core_vocabulary.py`: grep `core/` for a denylist (`palm`, `heart`, `life line`, `fate`, `mount`, `finger`, `gaze`, `saccade`…) with an allowlist file; run in `ci.yml`. (WP-X.1)

## 2. CI

- `ci.yml` was red on `dev` before the last ASD WPs (C++ build, mobile strings/shapes, coverage, sanitizers). Make it green or waive each failing job with a dated `# WAIVED 2026-…: reason` and an issue (WP-X.2, this week).
- Coverage job is `continue-on-error`; `coverage_gate.py` demands 100 % with a 10-line exemption list. Set a realistic gate (90 % lines on `core/`, measured on Linux) and make it blocking; the 100 % aspiration goes in docs (WP-X.2).
- Add `backend.yml` (WP-B.1) and console checks (WP-C.1). Path filters so iOS/backend/console don't block each other.
- Make `scripts/validate_hackathon_readiness.sh` print real numbers (after judging; F11).

## 3. Steger hysteresis normalisation (shared palm/flood bug) — WP-X.3

`convertTo(CV_32F)` leaves `|λ1|` unnormalised so configured low/high (0.5/1.5) are inert. Fix: scale-normalise the Hessian response (`σ²·λ`) and/or express thresholds as percentiles of the response histogram; keep both behaviours behind `StegerConfig.threshold_mode{raw, scale_normalised, percentile}` with `raw` as the default until palm and flood fixtures are re-swept, then flip. Tier S. Dec 2026.

## 4. Licensing and provenance

- Repo `LICENSE` is proprietary; `docs/license_policy.md` blocks "All Rights Reserved" for *dependencies* — fine, but say so explicitly in the policy to avoid confusing agents.
- `third_party/license_audit.md` has stale paths (`third_party/spline/`, `core/tests/`, `adapters/onnxruntime/` empty). Fix (WP-X.4).
- `models/README.md` references a missing `scripts/download_models.sh`; manifests/licences dirs empty. Create the script (hand-landmark ONNX for `adapters/opencv_dnn`) and a manifest schema; WP-D.3 uses it (WP-X.4).
- `data/fixtures/palm_fate/manifest.json` says images © palmistindian.com "do not redistribute" — but the repo is public. Remove those fixtures or confirm permission (WP-X.5, this week).
- `data/flood_labels/manifest.json` claims CC BY 4.0 for a Roboflow set whose images come from YouTube/screenshots; images are not tracked, keep it that way and note the caveat.
- Nebius would require Apache-2.0/MIT for a *separate* repo only (see `05 §3`).

## 5. Build hygiene

- Untracked-but-present clutter in root (`default.profraw`, `evidence.json`, `gemini_prompt.json`, `*.png`) is ignored by git; delete locally or move to `output/`. `.DS_Store` ignored.
- `apps/ios_asd_gaze_app/build-sim/` contains SPM checkouts — ensure ignored (U5).
- 295 MB `.git`; largest tracked blobs are two 16–17 MB overlay PNGs and personal photos (U3). Consider `git filter-repo` after judging (H-2) to drop them from history.
- `domains/README.md` omits `retina`, `asd_screening` and lists `driver_monitoring` as planned although it exists; `apps/README.md` omits `eval_tool` (empty dir — delete). (WP-X.4)

## 6. Work packages

| WP | Title | Tier | When |
|---|---|---|---|
| WP-X.1 | Core vocabulary gate + palm-specific moves | S | Dec 2026 |
| WP-X.2 | CI green/waivers; realistic coverage gate; path filters | I | this week |
| WP-X.3 | Steger threshold normalisation with mode switch + re-sweep | S | Dec 2026 |
| WP-X.4 | Licence audit paths, models download script + manifest schema, READMEs | I | Nov 2026 |
| WP-X.5 | Remove/confirm non-redistributable palm fixtures | I + owner | this week |
| WP-X.6 | `git filter-repo` purge of password file and personal photos (H-2) | S + owner | after 2026-11-10 |
