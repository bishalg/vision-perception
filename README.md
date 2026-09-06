# vision-perception

> **A Reusable, Domain-Agnostic Computer Vision Perception Core**

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Version: 0.1.0](https://img.shields.io/badge/Version-0.1.0-informational.svg)](CMakeLists.txt)
[![Architecture: Modular C++20](https://img.shields.io/badge/Architecture-Modular%20C%2B%2B20-orange.svg)](docs/architecture.md)
[![Milestone: 8 Complete](https://img.shields.io/badge/Milestone-8%20Complete-green.svg)](docs/roadmap.md)
[![CI](https://github.com/bishalg/vision-perception/actions/workflows/ci.yml/badge.svg)](.github/workflows/ci.yml)


---

## 1. Project Purpose

`vision-perception` is a production-grade, high-performance computer vision monorepo implemented primarily in modern C++ (C++20/C++17) and OpenCV.

Its primary capability is an unbiased, sub-pixel curvilinear ridge extraction engine grounded in differential geometry, scale-space Hessian tensor decomposition, and sub-pixel centerline tracking.

---

## 2. Reusable, Domain-Agnostic Vision Core

A foundational architectural invariant of this repository is that **the core must remain strictly domain-agnostic**:

1. **Agnostic Core (`core/`)**:
   - Operates purely on geometric, spatial, radiometric, and structural primitives: sub-pixel coordinates, curvilinear segments, topological junctions, Hessian eigenvalues, and image quality metrics.
   - Contains zero domain-specific heuristics, rules, or domain terminology.
2. **Domain Packs Added Later (`domains/`)**:
   - All application-specific logic will be added later as isolated domain extension packs.
   - Future domain packs include:
     - `domains/palm/`: Biometric hand landmark alignment and crease extraction.
     - `domains/poultry/`: Livestock density, dwell time, and behavior tracking.
     - `domains/driver_monitoring/`: Driver gaze, head pose, and drowsiness analysis.
     - `domains/surface_inspection/`: Micro-crack, scratch, and surface defect quality control.
     - `domains/document_scanning/`: Quadrilateral edge rectification and text line analysis.
3. **No Domain Pollution**:
   - Palmistry, astrology, Samudrik Shastra, poultry feeding, driver alertness thresholds, etc., are **strictly forbidden** inside `core/`.

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

## 5. Current Milestone Status

- [x] Milestone 1: Research & repo scaffold (architecture, licensing, evidence schema)
- [x] Milestone 2: CMake C++20/OpenCV skeleton with strict warnings and GoogleTest
- [x] Milestone 3: Core interfaces, geometry, and immutable evidence model
- [x] Milestone 4: Minimal image pipeline (Steger sub-pixel ridges + Laplacian quality gate)
- [x] Milestone 5: Overlay rendering, JSON evidence reports, CLI tools (`curv_cli`, `hand_inspector_cli`)
- [x] Stable C-ABI (`libcurv_capi`) with JNI (Android) and Swift (iOS) bindings source
- [x] **Milestone 6: Tests & evaluation tooling** — GitHub Actions CI (format, Ubuntu/macOS `-Werror` builds, ASan/UBSan, 100% line+branch coverage gate), clang-format/clang-tidy gates, `synth_generator` + `evaluate` RMSE < 0.1 px accuracy gate, `benchmark` latency profiles at 720p/1080p/4K (`docs/benchmarks.md`)
- [x] **Milestone 7: Geospatial Flood Perception Pack (OpenCV AI Competition 2026)** — Rapid glacier-collapse flood quantification in Nepal (Lende Khola / Bhote Koshi) using Sentinel-2 L2A 10m imagery (+88.4% flood signature surge, +121.1% channel widening, dual controls passing; see [`docs/flood_nepal_2026.md`](docs/flood_nepal_2026.md))
- [x] **Milestone 8: AWS Cloud Architecture & Special Award Targets** — AWS CDK v2 IaC (`aws_infra/`), two-tool Bedrock Agentic Vision loop (`getFloodEvidence` $\to$ `dispatchFloodAlert`), DynamoDB idempotency gate, dedicated S3 audit bucket, SQS fallback queue, and COOL on AWS Graviton3 benchmark suite (see [`docs/devpost_submission.md`](docs/devpost_submission.md) and [`docs/aws_benchmark_guide.md`](docs/aws_benchmark_guide.md))
- [ ] Next phase: mobile app packaging (Android Gradle/NDK + iOS SwiftPM/xcframework) and additional domain packs (poultry, driver_monitoring, document_scanning)
