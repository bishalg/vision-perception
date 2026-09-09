# Hackathon Presentation Pack — vision-perception

> **Audience**: OpenCV AI Competition / judges & demo-day  
> **Repo**: https://github.com/bishalg/vision-perception (`dev`)  
> **Demo tags**: `demo/phase-1` … `demo/phase-4`  
> **Total live walkthrough**: ~15 minutes (5 phases × ~2.5–3 min) or **5-minute** compressed Devpost video cut

---

## 0. How to use this pack

| Artifact | Use |
|---|---|
| **§1 Slide outline** | Paste into Google Slides / Keynote / Pitch (1 slide ≈ 1 row) |
| **§2 Live demo runbook** | Terminal + browser checklist the day of the talk |
| **§3 5-minute video cut** | Aligns with [`docs/devpost_submission.md`](devpost_submission.md) storyboard |
| **§4 Talking points & metrics** | Memorize these numbers; put them on title + impact slides |
| **§5 Backup / failure modes** | If Wi‑Fi or Caltrans dies mid-demo |

---

## 1. Slide outline (15–18 slides)

### Act A — Hook & problem (slides 1–3)

| # | Slide title | On-screen content | Speak (≈30–45s) |
|---|---|---|---|
| 1 | **Title** | `vision-perception` · Sub-pixel curvilinear perception & autonomous disaster response · OpenCV 5 + AWS Bedrock + COOL | Who you are + one-line mission |
| 2 | **The event** | Nepal · Lende Khola / Bhote Koshi · 26 Aug 2026 · before/after stills | Glacier collapse → flash flood; minutes matter |
| 3 | **Why pixels fail** | Clouds · sediment · shadows · 5-day satellite revisit | Need sub-pixel ridges + terrestrial CCTV + agentic dispatch |

### Act B — Science core (slides 4–6)

| # | Slide title | On-screen content | Speak |
|---|---|---|---|
| 4 | **Math in one slide** | Steger Hessian → Taylor zero-crossing · RMSE **&lt; 0.0301 px** (gate &lt; 0.1) | Differential geometry, not blob differencing |
| 5 | **Flood proof** | +**88.4%** signature · +**121.1%** width · spatial **0.151×** · temporal **0.905×** | Dual controls kill false alarms |
| 6 | **Architecture** | Fixtures → C++ adapters → pipeline → evidence JSON → Bedrock / Cesium | Point at phased ingestion Stages 1→3 |

### Act C — Live demo phases (slides 7–12) ← *presentation centrepiece*

| # | Slide title | Demo command / visual | Speak |
|---|---|---|---|
| 7 | **Phase 1 — Static CCTV** | `ctest` green + day/night/rain fixtures | Offline fixtures = reproducible science |
| 8 | **Phase 2 — Recorded video** | Side-by-side annotated AVI | Temporal ridges + FPS stride |
| 9 | **Phase 3 — Live snapshot** | `live_snapshot_demo --url mock://…` then optional `--direct` | Cache/proxy so demos never die on rate limits |
| 10 | **Phase 4 — 3D twin** | Cesium at `localhost:3000` · click I-80 + Bhote Koshi | Pose + still panels on globe |
| 11 | **Agentic Vision** | Bedrock 3-node trace diagram | `getFloodEvidence` → token → `dispatchFloodAlert` |
| 12 | **Use of COOL** | Graviton3 vs x86 table (−25% P50 · −32% $/scene) | Arm SIMD / Neoverse V1 |

### Act D — Close (slides 13–15)

| # | Slide title | On-screen content | Speak |
|---|---|---|---|
| 13 | **What ships today** | Milestones 1–12 checked · tags `demo/phase-*` · 24/24 tests | Production-minded monorepo |
| 14 | **Roadmap** | Phase 5 flood alert loop on twin · mobile packs | Honest “next” |
| 15 | **Ask / QR** | Repo URL · Devpost · contact | Thank judges · Q&A |

**Optional appendix slides**: evidence JSON schema · CDK stack diagram · license matrix · failure-mode table.

---

## 2. Live demo runbook (day-of)

### Pre-flight (30 min before)

```bash
git checkout dev && git pull
git describe --tags --always

# C++ demos
cmake --preset release && cmake --build --preset release -j
ctest --test-dir build --output-on-failure   # expect 24/24

# Phase 2 artifact (pre-render so talk isn’t waiting on encode)
mkdir -p output
./build/tools/video_pipeline_demo \
  --input data/fixtures/cctv_video/caltrans_i80_sample.avi \
  --output output/annotated_caltrans_i80.avi \
  --fps-stride 2 --side-by-side

# Phase 3 dry-run (offline-safe)
./build/tools/live_snapshot_demo \
  --url mock://caltrans_i80_day --interval-sec 1 --duration-sec 3

# Phase 4 twin
python3 tools/geojson_exporter.py
cd apps/cesium_viewer && npm install && npm run dev
# browser ready on http://localhost:3000
```

Keep **three windows** tiled: (1) terminal, (2) video player / Finder with annotated AVI, (3) Cesium browser.

### On-stage sequence (~12–15 min)

1. **Slide 2–3** (problem) — 1 min  
2. **Phase 1**: flash `ctest` summary + fixture PNGs — 2 min  
3. **Phase 2**: open `output/annotated_caltrans_i80.avi` — 2.5 min  
4. **Phase 3**: run mock live loop; if network OK, one `--direct caltrans_i80_drum` tick — 2.5 min  
5. **Phase 4**: Cesium fly-to + click Bhote Koshi pier — 2.5 min  
6. **Agentic + COOL slides** — 2 min  
7. **Close + QR** — 1 min  

### Compressed 5-minute cut (Devpost video)

Use the timing table in [`docs/devpost_submission.md`](devpost_submission.md) §2. Insert **15–20s** of Phase 2 side-by-side + **15s** Cesium click between “C++ core” and “Agentic” if judges care about CCTV/realtime narrative.

---

## 3. One-liners (memorize)

- “**Sub-pixel, not superpixel** — Steger RMSE under three-hundredths of a pixel.”  
- “**Dual controls** prove the flood is in the river, not in the clouds.”  
- “**Agent can’t freestyle** — cryptographic `evidenceToken` + DynamoDB idempotency.”  
- “**Demos don’t depend on Wi‑Fi** — Stage 1–2 fixtures + `mock://` + disk cache.”  
- “**Same C++ core** from laptop → Graviton3 COOL → Bedrock action loop.”

---

## 4. Metric cheatsheet (put on slides)

| Claim | Number | Source |
|---|---|---|
| Ridge accuracy | RMSE **0.0301 px** (gate &lt; 0.1) | synth → evaluate gate |
| Flood signature | **+88.4%** (1.884×) | Nepal Sentinel-2 pack |
| Channel width | **+121.1%** (19→42 px) | same |
| Spatial control | **0.151×** (&lt; 1.10 gate) | west slope window |
| Temporal control | **0.905×** | pre/post baseline |
| COOL P50 | **−25.4%** vs x86 | Graviton3 bench |
| Cost/scene | **−32.3%** | same |
| Test suite | **24/24** ctest | `dev` HEAD |
| Demo tags | `demo/phase-1`…`4` | GitHub |

---

## 5. Failure modes & backups

| Failure | Backup |
|---|---|
| No network | Phase 1–2 only; Phase 3 `mock://`; Cesium OSM tiles may need cache — pre-open twin once online |
| Caltrans rate-limit | Never open with `--direct` first; use mock then cache |
| Cesium blank globe | Hard-refresh; confirm `public/cameras.geojson` via `geojson_exporter.py` |
| Video player missing | `ffplay output/annotated_caltrans_i80.avi` or open in QuickTime |
| Build broken | Checkout `demo/phase-4` tag; show pre-recorded screen capture |

---

## 6. Suggested visual assets to capture (tonight)

Record short clips (10–20s each) for slides / backup video:

1. `ctest` ending on `100% tests passed`  
2. Phase 2 side-by-side AVI scrub  
3. Phase 3 terminal HUD scrolling mock ticks  
4. Cesium: zoom Himalaya → click pier → panel still  
5. (Optional) Bedrock CloudWatch 3-node trace from prior AWS demo  

Store under `docs/presentation/assets/` (git-ignored binaries OK) or external drive.

---

## 7. Milestone status (as of push)

| Milestone | Status | Tag |
|---|---|---|
| M9 Phase 1 static CCTV | Done | `demo/phase-1` |
| M10 Phase 2 video | Done | `demo/phase-2` |
| M11 Phase 3 HTTP snapshot | Done | `demo/phase-3` |
| M12 Phase 4 Cesium twin | Done | `demo/phase-4` |
| M13 Phase 5 flood alert on twin | Next | — |

**Remote**: https://github.com/bishalg/vision-perception/tree/dev
