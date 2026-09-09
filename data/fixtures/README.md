# Perception Test Fixtures & Gradual Ingestion Strategy

> **Directory**: `data/fixtures/`  
> **Lifecycle Status**: Active Test Fixtures  
> **Development Philosophy**: Gradual 3-Stage Development (Static Images $\to$ Recorded Video $\to$ Live Streams)

---

## 1. Gradual 3-Stage Development Strategy

To ensure rapid local development, zero network flakiness, and deterministic CI testing, feature implementations progress through three distinct stages:

```text
┌────────────────────────────────┐
│ Stage 1: Static Reference Img  │ ◄── [CURRENT STAGE]
│ • Local image files (.png/.jpg)│     Fastest local POC iteration
│ • Deterministic & reproducible │     Zero network latency / rate limits
│ • ctest & CI regression suite  │     Sub-pixel algorithm validation
└───────────────┬────────────────┘
                ▼
┌────────────────────────────────┐
│ Stage 2: Recorded Video Feeds  │
│ • Local short video (.mp4)     │     Temporal tracking & jitter analysis
│ • Offline simulated streaming  │     Ring-buffer & backpressure testing
│ • Frame-to-frame continuity    │     Dynamic waterline / vehicle flow
└───────────────┬────────────────┘
                ▼
┌────────────────────────────────┐
│ Stage 3: Live Real-Time Feeds  │
│ • Live RTSP / HLS / MJPEG      │     Production deployment
│ • Network reconnect & retry    │     Live AWS Bedrock Agentic Loop
│ • Edge & Cloud streaming       │     3D Cesium / GEV Digital Twin
└────────────────────────────────┘
```

---

## 2. Directory Structure

```text
data/fixtures/
├── cctv/                 # Highway & urban intersection reference snapshots (Caltrans, Austin, TfL)
│   ├── day_clear/        # Baseline daytime reference frames
│   ├── night_lowlux/     # Nighttime low-illumination and glare test frames
│   └── weather_rain/     # Rain, spray, and fog blur-gate test frames
├── cctv_flood/           # Alpine river corridor & bridge pier watermark snapshots (Nepal / Alpine)
│   ├── pre_flood/        # Baseline calm water stage
│   └── post_flood/       # High-turbidity flood surge & bank scour
└── satellite/            # Orthorectified optical (Sentinel-2) and SAR (Sentinel-1) reference crops
```

---

## 3. Quick Local POC Execution

Run the C++ Steger ridge extraction CLI against any static reference fixture:

```bash
# Test on a static CCTV reference snapshot
./build/tools/curv_cli --input data/fixtures/cctv/day_clear/caltrans_i80_sample.png \
                       --config configs/default_ridge.json \
                       --output data/out_cctv_overlay.png \
                       --evidence data/out_cctv_evidence.json

# Test on a static flood river corridor crop
./build/tools/curv_cli --input data/fixtures/cctv_flood/post_flood/bhote_koshi_sample.png \
                       --config configs/geospatial_wide_ridge.json \
                       --output data/out_flood_overlay.png \
                       --evidence data/out_flood_evidence.json
```
