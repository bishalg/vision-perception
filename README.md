# vision-perception

> **A Reusable, Domain-Agnostic Computer Vision Perception Core**

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Architecture: Modular C++20](https://img.shields.io/badge/Architecture-Modular%20C%2B%2B20-orange.svg)](docs/architecture.md)
[![Milestone: 1 Scaffold](https://img.shields.io/badge/Milestone-1%3A%20Scaffold-green.svg)](docs/roadmap.md)

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
│   ├── include/     # Public headers (curv/GeometryTypes.hpp, vision_core/version.hpp)
│   ├── src/         # Core implementation units
│   └── tests/       # Core unit tests
├── adapters/        # Technology & hardware adapters (OpenCV, ONNX Runtime, TFLite, MediaPipe, Camera)
├── domains/         # Domain extension packs (palm, poultry, driver_monitoring, surface_inspection, etc.)
├── apps/            # Application executables (desktop_pipeline_demo, eval_tool)
├── tools/           # Developer tools (run_pipeline, evaluate, overlay_viewer)
├── models/          # Neural network manifests, schemas, and download orchestrators
├── data/            # Sample fixtures, synthetic curves, and annotations
├── third_party/     # Permissive third-party libraries and provenance records
├── docs/            # Architecture specifications, evidence schemas, and research
│   └── research/    # Literature reviews, GitHub reuse matrices, benchmarks
└── scripts/         # Setup, build, test, lint, and formatting automation
```

---

## 4. Current Milestone Status

This project is currently in **Milestone 1: Repository Scaffold**:
- [x] Initialized Git repository tracking `git@github.com:bishalg/vision-perception.git`
- [x] Established multi-tier folder scaffold (`docs/`, `cmake/`, `core/`, `adapters/`, `domains/`, `apps/`, `tools/`, `models/`, `data/`, `third_party/`, `scripts/`)
- [x] Defined open-source reuse matrix and licensing governance (`docs/research/github_reuse_matrix.md`, `docs/license_policy.md`)
- [x] Authored architectural blueprints (`docs/architecture.md`, `docs/evidence_schema.md`, `docs/domain_pack_spec.md`)
- [x] Implemented core mathematical data contract (`core/include/curv/GeometryTypes.hpp`)
- [x] Configured modern CMake modules, `.gitignore`, and automation scripts
- [ ] *Next Phase*: Milestone 2 (CMake C++ OpenCV build verification and toolchain validation)

No business logic, heavy dependencies, palmistry heuristics, or mobile platform code have been implemented.
