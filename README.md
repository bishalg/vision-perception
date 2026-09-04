# Vision-Perception

> **A Domain-Agnostic Sub-Pixel Curvilinear Ridge Extraction Engine with Modular Domain Plugins**

[![License: Policy](https://img.shields.io/badge/License-Policy-blue.svg)](docs/license_policy.md)
[![Architecture: Modular C++20](https://img.shields.io/badge/Architecture-Modular%20C%2B%2B20-orange.svg)](docs/architecture.md)
[![Status: Research & Planning](https://img.shields.io/badge/Status-Research%20%26%20Planning-green.svg)](docs/roadmap.md)

---

## 1. Overview

`vision-perception` is a high-performance, domain-agnostic computer vision engine engineered primarily in modern C++ (C++20) and OpenCV. 

At the center of the engine is an unbiased, sub-pixel curvilinear structure and ridge extraction pipeline grounded in differential geometry, scale-space Hessian eigenvalue decomposition, and sub-pixel centerline tracking. 

The architecture strictly decouples mathematical and geometric perception primitives from downstream domain-specific semantics. Domain packs (e.g., palm feature extraction, poultry behavior tracking, industrial surface inspection, driver monitoring, document scanning) plug into the core via well-defined evidence models, adapters, and extension interfaces.

---

## 2. Core Philosophy & Design Principles

1. **Domain-Agnostic Core**: The core perception engine operates purely on geometric, spatial, radiometric, and structural primitives (points, ridges, contours, graphs, scale-space response, quality metrics). It contains zero domain-specific heuristics.
2. **Sub-Pixel Precision**: Ridges, edges, and junctions are extracted with sub-pixel localization using directional derivatives, Hessian matrix analysis, and zero-crossing Taylor approximations.
3. **Strict MVC / Model Decoupling**: View and rendering layers (overlays, visualizers) are isolated from core perception logic and mathematical models.
4. **Evidence-Based Output**: Every pipeline execution produces an immutable `PerceptionEvidence` model encapsulating geometry, quality assessment scores, confidence intervals, and execution telemetry suitable for JSON serialization.
5. **Zero Vendor Lock-In / Modular Adapters**: Hardware acceleration (OpenCV, ONNX Runtime, TFLite, Metal, Vulkan, OpenVINO) is abstracted through explicit adapter interfaces.
6. **Commercial Permissiveness**: Strict licensing hygiene ensuring only Apache-2.0, MIT, BSD-3-Clause, Boost, and zlib compatible components are integrated.

---

## 3. Monorepo Structure

```text
vision-perception/
├── core/            # Domain-agnostic vision core (ridge extraction, geometry, quality, evidence)
├── adapters/        # Runtime adapters (OpenCV, ONNX Runtime, TFLite, Camera/Video I/O)
├── domains/         # Domain packs (palm, poultry, surface_inspection, driver_monitoring, etc.)
├── apps/            # Applications (CLI testbench, evaluation daemon, demo runners)
├── tools/           # Developer tools (calibration, synthetic curve generator, benchmark harness)
├── models/          # Model manifests, schemas, and download orchestrators
├── data/            # Sample fixtures, calibration targets, and test artifacts
├── third_party/     # External dependencies (git submodules / vendored headers)
├── docs/            # Architecture specs, license policies, research benchmarks
│   └── research/    # Literature reviews, GitHub reuse matrices, competitive analysis
└── scripts/         # Environment setup, build helpers, CI/CD formatting & validation
```

---

## 4. Documentation

Detailed specifications and planning documentation are available in the [`docs/`](docs/) directory:

- [**GitHub Open-Source Reuse Matrix**](docs/research/github_reuse_matrix.md): Exhaustive audit of candidate open-source libraries for core CV, line detection, curve fitting, inference runtimes, and tracking.
- [**License & Provenance Policy**](docs/license_policy.md): Standards for third-party libraries, model weights, provenance tracking, and intellectual property protection.
- [**System Architecture Specification**](docs/architecture.md): Modular component breakdown, data flow, pipeline abstractions, evidence schema, and domain plugin contracts.
- [**Engineering Roadmap**](docs/roadmap.md): Milestone-based delivery plan from research and scaffolding through C++ implementation, evaluation tooling, and domain expansion.

---

## 5. Current Status

The project is currently in **Milestone 1 (Research and Repo Scaffold)**. No implementation code has been committed yet. The repository skeleton, git configuration, licensing framework, and architectural plans are established.

Proceed to [Roadmap](docs/roadmap.md) to inspect upcoming milestones.
