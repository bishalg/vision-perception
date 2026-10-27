# 05 — CurvFlood: Post-Hackathon Maintenance Roadmap

> OpenCV AI Competition 2026 entry submitted 2026-10-27. Judging 2026-10-27 → 11-09, winners 2026-11-10. **Rule for this window: do not force-push or restructure anything a judge link points at** (`README.md`, `docs/index.html`, `docs/JUDGES_QUICKSTART.md`, `scripts/validate_hackathon_readiness.sh`, `dist_submission/`). Fixes below land on a branch and merge after 2026-11-10 unless marked *safe now*.

## 1. State in one paragraph

Pipeline = Python/cv2 scripts (`prepare_flood_data.py` → `check_registration.py` ECC → `build/tools/curv_cli` Steger ridges → `compute_flood_delta.py` HSV water/sediment masks + channel-width proxy + spatial/temporal controls → `flood_delta.json`) + AWS CDK (`aws_infra/`: S3, DynamoDB, SNS/SQS, Lambda `agent_trigger.py`, Bedrock agent on Claude 3.5 Sonnet v2) + Cesium viewer + static portal. The C++ `GeospatialFloodPack` only classifies ridge segments by length. The "agentic trace" demo (`scripts/run_agentic_vision_demo.py`) is a simulation with no AWS calls. The COOL benchmark JSON is a local Mac run, not Graviton.

## 2. Defects to fix (after judging unless noted)

| # | Defect | Fix | WP |
|---|---|---|---|
| F1 | EventBridge-shaped event → 400 in `agent_trigger.py` | Parse `detail.bucket.name` / `detail.object.key`; add test with a real EventBridge sample | WP-F.1 |
| F2 | Synthetic positive fallback active unless `VISION_STRICT_MODE=1`; stack never sets it | Set `VISION_STRICT_MODE=1` in `agent_stack.py`; make strict the default and `VISION_DEMO_MODE=1` the opt-in | WP-F.1 |
| F3 | Token check skipped when DynamoDB item missing | Fail closed: no record → 403 | WP-F.1 |
| F4 | Idempotency get-then-update | `put_item` with `ConditionExpression attribute_not_exists(eventId)`; `update_item` with status condition | WP-F.1 |
| F5 | Lambda trusts agent's width/severity | Re-read `flood_delta.json` and re-apply the policy (width ≥ 2.0, spatial pass) server-side before SNS | WP-F.1 |
| F6 | Thresholds disagree (pipeline 1.10/1.25; prompt 2.0; demo 1.5/1.3; judges' doc "1.5x/0.20") | One `configs/flood_policy.json` read by `compute_flood_delta.py`, the Lambda (bundled) and the system prompt generator; docs cite it | WP-F.2 |
| F7 | `run_flood_pipeline.sh` hard-codes `SIGMA=5.0 LOW=0.2 HIGH=0.8 MIN_BLUR=15` vs config/docs 1.5/0.5/1.5 | Read `configs/geospatial_wide_ridge.json`; log the params into `flood_delta.json.provenance` | WP-F.2 |
| F8 | Docs vs artifacts (8,175/7,322 segments & −9.0 % in docs vs 172/138 & −17.65 % in JSON; COOL numbers from local Mac; F1 0.1426 vs 0.1362) | Regenerate `docs/flood_nepal_2026.md` and `PROGRESS.md` tables from JSON via a script (`scripts/render_flood_docs.py`), never by hand | WP-F.3 |
| F9 | Data not reproducible from a clean clone (`data/flood_nepal_2026/*` ignored) | Publish inputs to a public S3/Zenodo bucket with checksums; `scripts/fetch_flood_data.sh`; E2E test skips with a clear message when absent | WP-F.3 |
| F10 | Hysteresis thresholds effectively inert (unnormalised `|λ1|`) — HIGH in loophole register | Normalise Hessian response (divide by σ² scale-normalised max or use percentile thresholds) in `StegerRidgeExtractor`; re-sweep; this changes palm too, so gate with the palm fixtures | WP-X.3 (core) |
| F11 | Validator PASS strings are static ("41/41", "145 files") | Make the script print real counts; exit non-zero on any gate failure | WP-F.3 *(safe now? no — judges run it; after)* |
| F12 | Bedrock model id `claude-3-5-sonnet-20241022-v2` may be retired | Parameterise model id; default to a current Claude model; verify with `aws bedrock list-foundation-models` | WP-F.1 |
| F13 | Real Graviton COOL run never recorded | Run `scripts/benchmark_cool_vs_x86.sh` on c7g.2xlarge + c7i.2xlarge once (~$1), commit `data/benchmark_results_c7g.json` | WP-F.4 |
| F14 | Cesium viewer has no live `flood_delta` feed (D11) | Poll `events/*/flood_delta.json` from S3 (public read for demo) and draw the AOI polygon + severity badge | WP-F.5 |

## 3. Nebius × NVIDIA (deadline 2026-10-30 22:30 IST) — H-1

Needs: separate public Apache-2.0/MIT repo (`river-sentinel-nemotron`), Nemotron 3 via Nebius Token Factory (`tools/nebius_nemotron_agent.py` exists, 13 tests), 3-minute public video with ≥1 min of "physical" modules, pre-existing-work disclosure. Nothing ticked. Realistic only if the owner can spend the full three days on it. **Default: skip.** If pursued: copy `scripts/compute_flood_delta.py`, `tools/nebius_nemotron_agent.py`, `tools/waterline_gauge.py`, the CCTV adapter concept and a sample `flood_delta.json` into the new repo; do not move this repo to Apache-2.0 (the palm/ASD code is not meant to be open).

## 4. Where flood goes next (if it continues beyond maintenance)

Follow the existing `docs/flood_robust_roadmap.md` D1–D14 and `docs/flood_realtime_architecture.md` Phase 1–2, in this order and only after §2 is done:
1. **Holdout + metrics** (D1–D4): 20 Sentinel-2 pre/post pairs, `scripts/flood_metrics.py`, `docs/flood_baseline.md`. Without this there is no way to say the pipeline works beyond n=1.
2. **CCTV waterline gauge fusion** (Phase 1): `tools/waterline_gauge.py` already has 10 tests; wire into the delta JSON as a second signal.
3. **Sentinel-1 SAR** (D8–D9): no SAR code exists; sizeable.
4. **Robust change detection**: replace HSV colour masks with NDWI/MNDWI from Sentinel-2 bands (needs band-level inputs, not RGB PNG) — the biggest scientific improvement for the least code.

## 5. Work packages

| WP | Title | Tier | When |
|---|---|---|---|
| WP-F.1 | Lambda/agent hardening (F1–F5, F12) + tests | S | after 2026-11-10 |
| WP-F.2 | Single policy/params config (F6, F7) | I | after 2026-11-10 |
| WP-F.3 | Docs-from-artifacts, data fetch script, honest validator (F8, F9, F11) | I | after 2026-11-10 |
| WP-F.4 | Real Graviton benchmark | owner + I | any time (costs ~$1) |
| WP-F.5 | Cesium live feed | I | backlog |
| WP-F.6 | Holdout set + metrics | S | Dec 2026 if flood continues |
| WP-F.7 | NDWI-based water mask | S | Jan 2027 if flood continues |

Time budget suggestion: ≤ 1 day/week on flood until ASD Phase 2 is done.
