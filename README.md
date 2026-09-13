# vision-perception

> **A Reusable, Domain-Agnostic Computer Vision Perception Core**

[![License: Proprietary](https://img.shields.io/badge/License-Proprietary-red.svg)](LICENSE)
[![Version: 0.1.0](https://img.shields.io/badge/Version-0.1.0-informational.svg)](CMakeLists.txt)
[![Architecture: Modular C++20](https://img.shields.io/badge/Architecture-Modular%20C%2B%2B20-orange.svg)](docs/architecture.md)
[![Milestone: 8 Complete](https://img.shields.io/badge/Milestone-8%20Complete-green.svg)](docs/roadmap.md)
[![CI](https://github.com/bishalg/vision-perception/actions/workflows/ci.yml/badge.svg)](.github/workflows/ci.yml)


---

## 1. Project Purpose

`vision-perception` is a production-grade, high-performance computer vision monorepo implemented primarily in modern C++ (C++20/C++17) and OpenCV.

Its primary capability is an unbiased, sub-pixel curvilinear ridge extraction engine grounded in differential geometry, scale-space Hessian tensor decomposition, and sub-pixel centerline tracking.

Detection is the means, not the product: lines exist to produce **predictions** (palm readings, flood alerts). Humans stay in the loop — the app proposes, the human corrects fast, confirmations become training data. See [`docs/product_vision.md`](docs/product_vision.md).

**Labeling desk:** CurvStudio — `pnpm studio` → http://localhost:8080/apps/annotation_studio/. Operator steps (draw → save → compare → retune): [`docs/STUDIO_GUIDE.md`](docs/STUDIO_GUIDE.md).

---

## 2. Reusable, Domain-Agnostic Vision Core

A foundational architectural invariant of this repository is that **the core must remain strictly domain-agnostic**:

1. **Agnostic Core (`core/`)**:
   - Operates purely on geometric, spatial, radiometric, and structural primitives: sub-pixel coordinates, curvilinear segments, topological junctions, Hessian eigenvalues, and image quality metrics.
   - Contains zero domain-specific heuristics, rules, or domain terminology.
2. **Domain Packs Added Later (`domains/`)**:
   - All application-specific logic is isolated in domain extension packs.
   - Active and roadmap domain packs include:
     - `domains/palm/`: Biometric hand landmark alignment and crease extraction.
     - `domains/retina/`: Ophthalmology fundus micro-vessel segmentation, tortuosity index, and bifurcation tracking.
     - `domains/asd_screening/`: Pediatric social visual attention screening (GeoPref, Face Scanning, ResearchKit/CareKit).
     - `domains/driver_monitoring/`: Automotive in-cabin driver gaze, off-road distraction, and visual alertness.
     - `domains/geospatial/`: Satellite SAR / optical river corridor morphology and flood boundary quantification.
     - `domains/surface_inspection/`: Micro-crack, scratch, and surface defect quality control.
     - `domains/floor_plan/`: Wall polygon and architectural junction extraction.
     - `domains/document_scanning/`: Quadrilateral edge rectification and text line analysis.
3. **No Domain Pollution**:
   - Palmistry, astrology, medical diagnostics, poultry feeding, driver alertness thresholds, etc., are **strictly forbidden** inside `core/`.

---

## 3. Monorepo Structure

```text
vision-perception/
├── cmake/           # Modular CMake modules, toolchains, and compiler warning configurations
├── core/            # Domain-agnostic C++ perception core (geometry, ridge engine, quality, evidence)
│   ├── include/     # Public headers (curv/GeometryTypes.hpp, curv/pipeline/, curv/ridge/, ...)
│   └── src/         # Core implementation units
├── adapters/        # Technology & hardware adapters (OpenCV, OpenCV-DNN, C-ABI, JNI, Swift)
├── domains/         # Domain extension packs (palm, surface_inspection; poultry, driver_monitoring, document_scanning planned)
├── apps/            # Application executables (desktop_pipeline_demo)
├── tools/           # Developer tools (curv_cli, hand_inspector_cli, synth_generator, evaluate, benchmark)
├── tests/           # GoogleTest unit + regression suites (ctest)
├── models/          # Neural network manifests, schemas, and download orchestrators
├── data/            # Sample fixtures, synthetic curves, and annotations
├── third_party/     # Permissive third-party libraries and provenance records
├── docs/            # Architecture specifications, evidence schemas, and research
│   └── research/    # Literature reviews, GitHub reuse matrices, benchmarks
└── scripts/         # Setup, build, test, lint, and formatting automation
```

---

## 4. Build, Test & Quality Gates

```bash
cmake --preset release          # Release + warnings-as-errors (Ninja)
cmake --build --preset release -j
ctest --test-dir build          # full GoogleTest suite

cmake --preset asan && cmake --build --preset asan -j && ctest --test-dir build-asan   # ASan+UBSan
cmake --preset tsan && cmake --build --preset tsan -j && ctest --test-dir build-tsan   # TSan

./scripts/format.sh             # clang-format (in place; --check in CI)
./scripts/lint.sh               # clang-tidy static analysis
./scripts/build.sh              # configure + build + ctest + CLI smoke runs
```

GitHub Actions (`.github/workflows/ci.yml`) gates every push to `main`/`dev`: clang-format check, Ubuntu + macOS builds with `-Werror`, full ctest, clang-tidy, and an ASan/UBSan job.

---

## 5. Multi-Source Spatial & CCTV Perception

`vision-perception` integrates multi-modal spatial intelligence (drawing patterns from [`bilawalsidhu/gods-eye-view`](https://github.com/bilawalsidhu/gods-eye-view)) into its differential geometry pipeline via a **gradual 3-stage development model**:

```text
Stage 1: Static Images ✅          ──►  Stage 2: Recorded Video ✅  ──►  Stage 3: Live Streams (Current)
• Local .png/.jpg fixtures              • Local short .avi/.mp4 clips      • HTTP snapshot pull + cache
• Instant, reproducible local POC       • FPS-stride + annotated demo CLI  • mock:// offline presentation mode
• Zero network dependencies             • Ring-buffer backpressure ready   • Optional --direct live Caltrans
```

### Ingestion Matrix & Documentation
| Domain / Source | Format & Feeds | Technical Documentation |
| :--- | :--- | :--- |
| **Real-Time Flood (Nepal / Alpine)** | Sentinel-1 SAR + Sentinel-2 L2A + River CCTV | [`docs/flood_realtime_architecture.md`](docs/flood_realtime_architecture.md) |
| **Highway & Urban CCTV** | Caltrans, Austin, TfL (HLS / JPEG snapshots) | [`docs/research/gods_eye_view_and_cctv_integration.md`](docs/research/gods_eye_view_and_cctv_integration.md) |
| **Pediatric Social Attention & ASD Screening** | TrueDepth ARKit Gaze + Multi-Task Stimuli | [`domains/asd_screening/README.md`](domains/asd_screening/README.md) |
| **Retinal Micro-Vascular Biometrics** | Fundus Photography (Green Channel CLAHE) | [`domains/retina/README.md`](domains/retina/README.md) |
| **System Spatial Architecture** | Multi-tier pipeline & 2D-to-3D viewshed projection | [`docs/research/realtime_spatial_perception_architecture.md`](docs/research/realtime_spatial_perception_architecture.md) |
| **Test Fixtures & Local POC** | Curated reference datasets for rapid offline dev | [`data/fixtures/README.md`](data/fixtures/README.md) |
| **Research Index** | High-density discovery index for humans & AI | [`docs/research/README.md`](docs/research/README.md) |
| **Hackathon presentation** | Slide outline, live demo runbook, metric cheatsheet | [`docs/hackathon_presentation.md`](docs/hackathon_presentation.md) |

---

## 6. Current Milestone Status

- [x] Milestone 1: Research & repo scaffold (architecture, licensing, evidence schema)
- [x] Milestone 2: CMake C++20/OpenCV skeleton with strict warnings and GoogleTest
- [x] Milestone 3: Core interfaces, geometry, and immutable evidence model
- [x] Milestone 4: Minimal image pipeline (Steger sub-pixel ridges + Laplacian quality gate)
- [x] Milestone 5: Overlay rendering, JSON evidence reports, CLI tools (`curv_cli`, `hand_inspector_cli`)
- [x] Stable C-ABI (`libcurv_capi`) with JNI (Android) and Swift (iOS) bindings source
- [x] **Milestone 6: Tests & evaluation tooling** — GitHub Actions CI (format, Ubuntu/macOS `-Werror` builds, ASan/UBSan, 100% line+branch coverage gate), clang-format/clang-tidy gates, `synth_generator` + `evaluate` RMSE < 0.1 px accuracy gate, `benchmark` latency profiles at 720p/1080p/4K (`docs/benchmarks.md`)
- [x] **Milestone 7: Geospatial Flood Perception Pack (OpenCV AI Competition 2026)** — Rapid glacier-collapse flood quantification in Nepal (Lende Khola / Bhote Koshi) using Sentinel-2 L2A 10m imagery (+88.4% flood signature surge, +121.1% channel widening, dual controls passing; see [`docs/flood_nepal_2026.md`](docs/flood_nepal_2026.md))
- [x] **Milestone 8: AWS Cloud Architecture & Special Award Targets** — AWS CDK v2 IaC (`aws_infra/`), two-tool Bedrock Agentic Vision loop (`getFloodEvidence` $\to$ `dispatchFloodAlert`), DynamoDB idempotency gate, dedicated S3 audit bucket, SQS fallback queue, and COOL on AWS Graviton3 benchmark suite (see [`docs/devpost_submission.md`](docs/devpost_submission.md) and [`docs/aws_benchmark_guide.md`](docs/aws_benchmark_guide.md))
- [x] **Milestone 9: Real-Time CCTV & Spatial Perception (Phase 1: Static Reference POC)** — Curate `data/fixtures/cctv/` reference images, implement `adapters/cctv/`, and evaluate Steger curvilinear extraction on highway lane lines and river corridor watermarks
- [x] **Milestone 10: Phase 2 Recorded Video Pipeline** — `CctvVideoSource` + `video_pipeline_demo` (quality + Steger annotated AVI/MP4), ring-buffer stream adapter foundation, offline `data/fixtures/cctv_video/` clip
- [x] **Milestone 11: Phase 3 Live HTTP Snapshot Stream** — `CctvHttpSnapshot` + `live_snapshot_demo` with local cache/proxy fallback for presentation-safe demos
- [x] **Milestone 12: Phase 4 CesiumJS 3D Digital Twin** — `apps/cesium_viewer` (Vite + Cesium, OSM tiles) + `tools/geojson_exporter.py` frustum markers and still panels
- [x] **Milestone 13: Retinal & Pediatric ASD Screening Perception Packs** — Steger fundus micro-vessel segmentation (`domains/retina/`), core temporal oculomotor dynamics (`core/temporal/`: I-VT, I-DT, DTW), and pediatric social visual attention pack (`domains/asd_screening/`) with ResearchKit and CareKit data schemas. Specifications:
  - Architecture: [`domains/asd_screening/docs/ARCHITECTURE.md`](domains/asd_screening/docs/ARCHITECTURE.md)
  - Stimuli & Dynamic AOIs: [`domains/asd_screening/docs/STIMULI_SPEC.md`](domains/asd_screening/docs/STIMULI_SPEC.md)
  - Data Collection & Ethics: [`domains/asd_screening/docs/ETHICS_PROTOCOL.md`](domains/asd_screening/docs/ETHICS_PROTOCOL.md)
  - ML & Validation Roadmap: [`domains/asd_screening/docs/ML_ROADMAP.md`](domains/asd_screening/docs/ML_ROADMAP.md)
  - Literature Survey & 12-Phase Plan: [`domains/asd_screening/docs/LITERATURE.md`](domains/asd_screening/docs/LITERATURE.md)
- [ ] **Milestone 14: Phase 5 Flood Alert Production Demo** — scheduled ingest → flood heuristics → alert banner on twin
- [ ] Next phase: mobile packaging remains parallel track


