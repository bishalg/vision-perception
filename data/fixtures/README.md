# Perception Test Fixtures & Gradual Ingestion Strategy

> **Directory**: `data/fixtures/`  
> **Lifecycle Status**: Active Test Fixtures  
> **Development Philosophy**: Gradual 3-Stage Development (Static Images → Recorded Video → Live Streams)

---

## 1. Gradual 3-Stage Development Strategy

```text
┌────────────────────────────────┐
│ Stage 1: Static Reference Img  │ ✅ DONE
│ • Local image files (.png/.jpg)│
│ • Deterministic & reproducible │
│ • ctest & CI regression suite  │
└───────────────┬────────────────┘
                ▼
┌────────────────────────────────┐
│ Stage 2: Recorded Video Feeds  │ ✅ DONE
│ • Local short video (.avi/.mp4)│
│ • Offline simulated streaming  │
│ • Annotated demo CLI output    │
└───────────────┬────────────────┘
                ▼
┌────────────────────────────────┐
│ Stage 3: Live Snapshot Pull    │ ◄── [CURRENT STAGE]
│ • HTTP JPEG/PNG snapshots      │     Caltrans / Austin / mock://
│ • Disk cache for presentations │     live_snapshot_demo
│ • Optional --direct live mode  │
└────────────────────────────────┘
```

---

## 2. Directory Structure

```text
data/fixtures/
├── cctv/                 # Highway & urban intersection reference snapshots
│   ├── day_clear/
│   ├── night_lowlux/
│   ├── weather_rain/
│   └── video/            # Stream-adapter soak clip (ring buffer tests)
├── cctv_video/           # Phase 2 recorded-video demo + unit-test clip
│   ├── caltrans_i80_sample.avi
│   └── caltrans_i80_sample.json
├── cctv_flood/           # Alpine river corridor watermark snapshots
└── satellite/            # Orthorectified optical / SAR reference crops
```

---

## 3. Quick Local POC Execution

```bash
# Stage 1 — static CCTV snapshot
./build/tools/curv_cli --image data/fixtures/cctv/day_clear/caltrans_i80_day.png \
                       --out data/out_cctv_overlay.png \
                       --json data/out_cctv_evidence.json

# Stage 2 — recorded video → annotated output (side-by-side raw | Steger)
./build/tools/video_pipeline_demo \
  --input data/fixtures/cctv_video/caltrans_i80_sample.avi \
  --output output/annotated_caltrans_i80.avi \
  --fps-stride 2 \
  --side-by-side

# Stage 3 — live / mock HTTP snapshot loop (presentation-safe with mock://)
./build/tools/live_snapshot_demo \
  --url mock://caltrans_i80_day \
  --interval-sec 2 \
  --duration-sec 10

# Live Caltrans pull (uses cache fallback on failure; --direct skips warm cache)
./build/tools/live_snapshot_demo \
  --url caltrans_i80_drum \
  --interval-sec 5 \
  --duration-sec 30 \
  --direct
```
