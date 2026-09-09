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
│ Stage 2: Recorded Video Feeds  │ ◄── [CURRENT STAGE]
│ • Local short video (.avi/.mp4)│     Temporal tracking & FPS stride
│ • Offline simulated streaming  │     Ring-buffer & backpressure ready
│ • Annotated demo CLI output    │     video_pipeline_demo
└───────────────┬────────────────┘
                ▼
┌────────────────────────────────┐
│ Stage 3: Live Real-Time Feeds  │
│ • Live RTSP / HLS / MJPEG      │
│ • HTTP snapshot pull (Phase 3) │
│ • Edge & Cloud streaming       │
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

# Regenerate synthetic video fixture (no network)
python3 tools/cctv_video_fixture.py \
  -o data/fixtures/cctv_video/caltrans_i80_sample.avi
```
