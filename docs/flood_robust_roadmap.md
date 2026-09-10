# Flood & River Morphology Robust 2-Week Roadmap — Day-by-Day

**Date:** 2026-09-10
**Status:** Active — mirrors `docs/palm_robust_roadmap.md:1` for parallel tracking
**Core:** Same `core/src/ridge/StegerRidgeExtractor.cpp:1` sub-pixel engine as palm; domain isolates in `domains/geospatial` + `adapters/cctv`

---

## Goal: Progressive, measurable flood detection + morphology + CCTV God-Eye

* **Surge:** `flood_delta.json` `flood_signature_ratio ≥1.10` stable across 20 holdout scenes (currently `1.884` on `post_20260827`)
* **Width:** median `W` `≥1.25×` with `±3px` repeatability (currently `2.21×` 19→42px)
* **Controls:** spatial `<1.10` (0.151) + temporal `<1.10` (0.905) always PASS
* **Latency:** `Steger 512 warp + CctvFrameSource` <120ms on Graviton3 `c7g`

---

## Week 1 — Ground Truth & Sweep (desktop, no Cloud deploy)

* **D1:** Curate `data/fixtures/flood_holdout/` 20 pairs: 10 flood +10 dry from `data/flood_nepal_2026/raw/` + `data/fixtures/cctv_flood/` pier crops. Add `labelme` river_polygon + waterline polyline. Script `tools/flood_label_export.py`.
* **D2:** Freeze metric `scripts/flood_metrics.py --holdout` → `IoU/surge/width` per scene; commit `docs/flood_baseline.md` snapshot (current `+88.4%/+121%`).
* **D3-4:** Sweep `configs/geospatial_wide_ridge.json:1` `σ 1.5/3.0/5.0` × `low 0.3-0.7` via `hand_inspector_cli` loop; log `docs/flood_sweep.md` best that keeps `segments <400` and `surge>1.4` on all flood scenes.
* **D5:** Fix gate: `adapters/capi/src/curv_capi.cpp:483` quality `is_usable` flag + `min_blur 15→12` for SAR speckle; add `tests/unit/test_geospatial_pack.cpp:1` holdout assert `surge>1.25`.

## Week 2 — SAR + CCTV + Twin (field & Cloud)

* **D8-9:** SAR: `adapters/cctv/src/CctvFrameSource.cpp:1` Enhanced Lee `3×3` + `VV/VH` backscatter `threshold -14dB` under cloud mask; verify on monsoon `2026-08-12` cloudy tile (cloud 8.8% in `flood_nepal_2026.md:92`).
* **D10:** CCTV pier waterline: `adapters/cctv/CctvStreamAdapter.cpp:1` Steger on pier marker `H=6.2m` + `Rasuwagadhi` choke; calibrate `H_c, θ_tilt` per `flood_realtime_architecture.md:125`; test on `data/fixtures/cctv_video/clip.avi`.
* **D11:** Twin: `apps/cesium_viewer/src/main.js:1` WebSocket `flood_delta.json` → extruded water volume `W(y)` + frustum `geojson_exporter.py`; verify `2s` refresh.
* **D12:** Cloud smoke: `aws_infra/stacks/storage_stack.py:1` `cdk synth` + `agent_trigger.py` unit `8/8`; local `benchmark_cool_vs_x86.sh` dry-run.
* **D14:** Final audit: `ctest 29/29`, `hand_inspector_cli` 10 flood +10 dry, `width±3px`, controls PASS, tag `flood-v1.0`.

Track daily in `docs/PROGRESS.md` + `data/benchmark_results.json`.
