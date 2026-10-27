# CurvFlood: Sub-Pixel Vision & Agentic Flood AI
> **OpenCV AI Competition 2026 (Powered by AWS) — Real-World Impact Track**  
> *Targeting: **Agentic Vision Award** & **Best Use of COOL Award***

[![Devpost Submission](https://img.shields.io/badge/Devpost-Submission_Page-003E54?style=for-the-badge&logo=devpost&logoColor=white)](https://devpost.com/software/curvflood-sub-pixel-vision-agentic-flood-ai)
[![Demo Video](https://img.shields.io/badge/YouTube-5_Min_Demo_Video-FF0000?style=for-the-badge&logo=youtube&logoColor=white)](https://youtu.be/XIoCuDY5gIw)
[![Google Slides](https://img.shields.io/badge/Google_Slides-Presentation_Deck-F4B400?style=for-the-badge&logo=google-slides&logoColor=white)](https://docs.google.com/presentation/d/1xkBtj0Dcbu0qqpKt-i9HooKt_OlyqlSv_nXqCn1As0c/edit)
[![Live Web Portal](https://img.shields.io/badge/Live_Portal-GitHub_Pages-22c55e?style=for-the-badge&logo=githubpages&logoColor=white)](https://bishalg.github.io/vision-perception/)
[![Architecture: Modular C++20](https://img.shields.io/badge/Architecture-Modular%20C%2B%2B20-orange.svg)](docs/architecture.md)
[![Tests: 41/41 Green](https://img.shields.io/badge/CTest-41%2F41%20PASS-brightgreen.svg)](scripts/validate_hackathon_readiness.sh)

| Quick Link | Destination |
|---|---|
| 🎬 **Official 5-Minute Video** | [Watch on YouTube (5:00)](https://youtu.be/XIoCuDY5gIw) |
| 📑 **Devpost Public Project** | [CurvFlood on Devpost](https://devpost.com/software/curvflood-sub-pixel-vision-agentic-flood-ai) |
| 📊 **Slides Presentation** | [Google Slides Deck](https://docs.google.com/presentation/d/1xkBtj0Dcbu0qqpKt-i9HooKt_OlyqlSv_nXqCn1As0c/edit) · [Interactive HTML Deck](docs/presentation/slides.html) |
| ⚡ **Judges' 60-Second Audit** | Run `./scripts/validate_hackathon_readiness.sh` (Zero cloud credentials required) |
| 🌐 **Interactive 3D Digital Twin** | Run `npm run dev` → Open [http://localhost:3000](http://localhost:3000) |

---

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

Detection is the means, not the product: lines exist to produce **actionable evidence** (such as automated glacier-collapse flood quantification and emergency alerts). Humans stay in the loop with automated verification gates.

**Labeling desk:** CurvStudio — `pnpm studio` → http://localhost:8080/apps/annotation_studio/. Operator steps (draw → save → compare → retune): [`docs/STUDIO_GUIDE.md`](docs/STUDIO_GUIDE.md).

---

## 2. Reusable, Domain-Agnostic Vision Core

A foundational architectural invariant of this repository is that **the core must remain strictly domain-agnostic**:

1. **Agnostic Core (`core/`)**:
   - Operates purely on geometric, spatial, radiometric, and structural primitives: sub-pixel coordinates, curvilinear segments, topological junctions, Hessian eigenvalues, and image quality metrics.
   - Contains zero domain-specific heuristics, rules, or domain terminology.
2. **Domain Packs (`domains/`)**:
   - All application-specific logic is isolated in modular domain extension packs.
   - Active and roadmap domain packs include:
     - `domains/geospatial/`: Satellite optical & SAR river corridor morphology and flood boundary quantification (featured in OpenCV AI 2026).
     - `domains/surface_inspection/`: Micro-crack, scratch, and surface defect quality control.
     - `domains/floor_plan/`: Wall polygon and architectural junction extraction.
     - `domains/document_scanning/`: Quadrilateral edge rectification and text line analysis.
3. **No Domain Pollution**:
   - Domain-specific heuristics and thresholds are **strictly forbidden** inside `core/`.

---

## 3. Monorepo Structure

```text
vision-perception/
├── cmake/           # Modular CMake modules, toolchains, and compiler warning configurations
├── core/            # Domain-agnostic C++ perception core (geometry, ridge engine, quality, evidence)
│   ├── include/     # Public headers (curv/GeometryTypes.hpp, curv/pipeline/, curv/ridge/, ...)
│   └── src/         # Core implementation units
├── adapters/        # Technology & hardware adapters (OpenCV, OpenCV-DNN, C-ABI, CCTV stream)
├── domains/         # Domain extension packs (geospatial flood monitoring, surface inspection, etc.)
├── apps/            # Application executables (cesium_viewer, desktop_pipeline_demo)
├── tools/           # Developer tools (curv_cli, synth_generator, evaluate, benchmark)
├── tests/           # GoogleTest unit + regression suites (ctest)
├── models/          # Neural network manifests, schemas, and download orchestrators
├── data/            # Sample fixtures, Sentinel-2 disaster imagery, and ground truth annotations
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

### 4.1 Pinned Dependencies (tested versions)

| Dependency | Pinned / tested version |
| :--- | :--- |
| OpenCV | **5.0.0** (verified `cv2.__version__`; Homebrew `/opt/homebrew/opt/opencv`, or build from source with `-DOPENCV_ENABLE_NONFREE=ON`) |
| CMake / Ninja | ≥ 3.25 (tested 4.4.3) / 1.13.2 |
| C++ compiler | C++20 — Apple Clang 21, Clang ≥ 18, or GCC ≥ 13 |
| Python (scripts + CDK) | ≥ 3.9 (tested 3.9.6) |
| `aws-cdk-lib` / `constructs` | **== 2.255.0 / == 10.6.0** — pinned in [`aws_infra/requirements-cdk.txt`](aws_infra/requirements-cdk.txt); offline `cdk synth` verified (all 4 stack templates generate) |
| `boto3` / `pytest` | == 1.42.97 / == 8.4.2 |
| Node.js (annotation studio / Cesium twin) | ≥ 20 (tested 22.22.0) |

AWS CDK offline verification (no AWS account required):

```bash
python3 -m venv .venv-cdk && .venv-cdk/bin/pip install -r aws_infra/requirements-cdk.txt
cd aws_infra && PATH="$PWD/../.venv-cdk/bin:$PATH" npx aws-cdk@2 synth   # expect cdk.out/*.template.json
```

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
- [x] Milestone 5: Overlay rendering, JSON evidence reports, CLI tools (`curv_cli`)
- [x] Stable C-ABI (`libcurv_capi`)
- [x] **Milestone 6: Tests & evaluation tooling** — GitHub Actions CI (format, Ubuntu/macOS `-Werror` builds, ASan/UBSan, 100% line+branch coverage gate), clang-format/clang-tidy gates, `synth_generator` + `evaluate` RMSE < 0.1 px accuracy gate, `benchmark` latency profiles at 720p/1080p/4K (`docs/benchmarks.md`)
- [x] **Milestone 7: Geospatial Flood Perception Pack (OpenCV AI Competition 2026)** — Rapid glacier-collapse flood quantification in Nepal (Lende Khola / Bhote Koshi) using Sentinel-2 L2A 10m imagery (+88.4% flood signature surge, +121.1% channel widening, dual controls passing; see [`docs/flood_nepal_2026.md`](docs/flood_nepal_2026.md))
- [x] **Milestone 8: AWS Cloud Architecture & Special Award Targets** — AWS CDK v2 IaC (`aws_infra/`), two-tool Bedrock Agentic Vision loop (`getFloodEvidence` $\to$ `dispatchFloodAlert`), DynamoDB idempotency gate, dedicated S3 audit bucket, SQS fallback queue, and COOL on AWS Graviton3 benchmark suite (see [`docs/devpost_submission.md`](docs/devpost_submission.md) and [`docs/aws_benchmark_guide.md`](docs/aws_benchmark_guide.md))
- [x] **Milestone 9: Real-Time CCTV & Spatial Perception (Phase 1: Static Reference POC)** — Curate `data/fixtures/cctv/` reference images, implement `adapters/cctv/`, and evaluate Steger curvilinear extraction on highway lane lines and river corridor watermarks
- [x] **Milestone 10: Phase 2 Recorded Video Pipeline** — `CctvVideoSource` + `video_pipeline_demo` (quality + Steger annotated AVI/MP4), ring-buffer stream adapter foundation, offline `data/fixtures/cctv_video/` clip
- [x] **Milestone 11: Phase 3 Live HTTP Snapshot Stream** — `CctvHttpSnapshot` + `live_snapshot_demo` with local cache/proxy fallback for presentation-safe demos
- [x] **Milestone 12: Phase 4 CesiumJS 3D Digital Twin** — `apps/cesium_viewer` (Vite + Cesium, OSM tiles) + `tools/geojson_exporter.py` frustum markers and still panels
- [x] **Milestone 13: Phase 5 Flood Alert Production Demo** — scheduled ingest → flood heuristics → alert banner on twin with zero-credential reproduction gate (`scripts/validate_hackathon_readiness.sh`)


