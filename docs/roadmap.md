# Engineering Roadmap

> **Target Repository**: `vision-perception`  
> **Lifecycle Status**: Active Implementation Roadmap  
> **Current Milestone**: M1–M6 complete — next: Mobile App Packaging (Phase C)  

---

## 1. Roadmap Overview

The `vision-perception` platform follows a phased, test-driven milestone delivery plan. Each milestone builds incrementally upon verified primitives, guaranteeing high code quality, commercial licensing compliance, and sub-pixel accuracy.

```mermaid
gantt
    title Vision-Perception Engineering Milestones
    dateFormat  YYYY-MM-DD
    section Scaffolding
    M1: Research & Repo Scaffold       :done,    m1, 2026-09-01, 2026-09-04
    M2: CMake C++ OpenCV Skeleton      :done,    m2, 2026-09-05, 2026-09-12
    section Core Perception
    M3: Core Interfaces & Evidence     :done,    m3, 2026-09-13, 2026-09-20
    M4: Minimal Image Pipeline         :done,    m4, 2026-09-21, 2026-10-05
    section Delivery & Tooling
    M5: Overlay & JSON Report          :done,    m5, 2026-10-06, 2026-10-15
    M6: Tests & Evaluation Tooling     :done,    m6, 2026-10-16, 2026-10-30
    M7: Mobile App Packaging (Phase C) :active,  m7, 2026-10-31, 2026-11-20
```

---

## 2. Milestone Details

### Milestone 1: Research and Repo Scaffold
*Status: DONE*

- **Objectives**:
  - Audit open-source libraries for permissive reuse vs clean-room implementation.
  - Establish enterprise licensing, provenance, and model weight governance.
  - Define domain-agnostic architecture, directory boundaries, and evidence schema.
  - Initialize Git repository, `.gitignore`, and directory skeleton.
- **Key Deliverables**:
  - `docs/research/github_reuse_matrix.md` (audit of OpenCV, ONNX, tracking, splines).
  - `docs/license_policy.md` (greenlist/redlist, model weights, provenance rules).
  - `docs/architecture.md` (agnostic core, adapters, domain plugins, evidence model).
  - `docs/roadmap.md` (this delivery plan).
  - `.gitignore` covering C++, CMake, Apple builds, Android NDK, models, and datasets.
  - Placeholder `README.md` introducing the sub-pixel ridge engine.
  - Directory skeleton with `.gitkeep` placeholders.
- **Exit Criteria**:
  - Documentation peer-reviewed and formally approved by the repository owner.
  - Clean git status with remote tracking `git@github.com:bishalg/vision-perception.git`.

---

### Milestone 2: CMake C++ OpenCV Skeleton
*Status: DONE*

- **Objectives**:
  - Establish modern C++20 modular CMake build system.
  - Configure target-based dependency discovery for OpenCV (`opencv_core`, `opencv_imgproc`, `opencv_calib3d`).
  - Configure compiler flags (`-Wall -Wextra -Wpedantic -Wconversion`, sanitizers, optimizations).
  - Set up GoogleTest unit testing framework.
- **Key Deliverables**:
  - Root `CMakeLists.txt` enforcing C++20 standard, target export, and project versioning.
  - Sub-module `CMakeLists.txt` for `core/`, `adapters/`, `apps/`, and `tests/`.
  - Smoke test binary linking against OpenCV and verifying version and matrix operations.
  - Environment bootstrap script `scripts/setup_env.sh` (macOS Homebrew and Ubuntu APT).
- **Exit Criteria**:
  - Clean compilation on local developer workstation (`cmake --preset` or `cmake -B build`).
  - Zero compiler warnings with all diagnostic flags enabled.
  - Automated smoke test passes via `ctest`.

---

### Milestone 3: Core Interfaces and Evidence Model
*Status: DONE*

- **Objectives**:
  - Implement fundamental geometric primitives and immutable evidence structures.
  - Define abstract pipeline stage and telemetry contracts.
  - Implement JSON serialization/deserialization for perception results.
- **Key Deliverables**:
  - `core/geometry/`: `Point2DSubpixel`, `BoundingBox2D`, `Polyline2D`, `SplineCurve2D`.
  - `core/quality/`: `QualityMetric`, `FocusScore`, `IlluminationProfile`.
  - `core/evidence/`: `PerceptionEvidence` model, telemetry aggregators.
  - `core/pipeline/`: `IPipelineStage`, `PipelineContext`, `PipelineResult`.
  - Comprehensive unit test suite covering schema serialization and immutable value semantics.
- **Exit Criteria**:
  - 100% unit test coverage on all core geometry and evidence classes.
  - Serialization to/from JSON roundtrips without numerical precision loss.

---

### Milestone 4: Minimal Image Pipeline
*Status: DONE*

- **Objectives**:
  - Implement the mathematical sub-pixel curvilinear ridge detection core (Steger algorithm).
  - Implement the real-time quality gating stage (Laplacian focus variance, illumination bounds).
  - Assemble the end-to-end processing pipeline orchestrator.
- **Key Deliverables**:
  - `core/quality/focus_gate`: Fast Laplacian variance filter with configurable thresholds.
  - `core/ridge/scale_space`: Multi-scale 2D Gaussian derivative convolution ($r_x, r_y, r_{xx}, r_{xy}, r_{yy}$).
  - `core/ridge/hessian_detector`: Pixel-wise Hessian eigenvalue decomposition and sub-pixel directional Taylor expansion.
  - `core/ridge/hysteresis_linker`: Dual-threshold ridge tracking and polyline assembly.
  - `core/pipeline/vision_pipeline`: Synchronous DAG orchestrator executing Quality Gate -> Ridge Extraction -> Evidence Assembly.
- **Exit Criteria**:
  - Pipeline successfully extracts sub-pixel ridges from a static test image.
  - Quality gate correctly rejects severely out-of-focus or black frames.

---

### Milestone 5: Overlay and JSON Report
*Status: DONE (delivered as `curv_cli` and `hand_inspector_cli` rather than `apps/cli/vision_run`)*

- **Objectives**:
  - Implement high-visibility visualization and rendering adapters.
  - Build standalone CLI utility for processing static images, directories, and video feeds.
  - Export standardized machine-readable JSON perception reports.
- **Key Deliverables**:
  - `adapters/opencv/overlay_renderer`: Anti-aliased sub-pixel drawing (ridge centerlines, normal vectors, width ribbons, quality HUD).
  - `core/evidence/json_reporter`: High-performance JSON report emitter.
  - `apps/cli/vision_run`: Production CLI executable supporting:
    - `--input <path>` (image or video file)
    - `--output-overlay <path>` (rendered visual verification artifact)
    - `--output-json <path>` (structured perception evidence report)
    - `--sigma <float>` (scale-space ridge smoothing parameter)
    - `--low-thresh <float>` / `--high-thresh <float>` (hysteresis sensitivity)
- **Exit Criteria**:
  - CLI binary executes on sample images, producing visual overlay and JSON report matching specifications.

---

### Milestone 6: Tests and Evaluation Tooling
*Status: DONE (2026-09-06)*

- **Objectives**:
  - Build synthetic ground-truth evaluation harness.
  - Validate sub-pixel precision down to sub-0.1 pixel accuracy against known mathematical curves.
  - Conduct micro-benchmarks and latency profiling across frame resolutions.
  - Finalize repository CI/CD automation.
- **Key Deliverables** (as landed):
  - `tools/synth_generator` (+ `tools/lib/curv/tools/synth_lib`): synthetic curve generator rendering lines, parabolas, and spirals with known analytical ground truth and parameterized Gaussian noise.
  - `tools/evaluate` (+ `evaluate_lib`): error calculator measuring RMSE, sub-pixel localization bias, and false positive/negative rates; enforced in CI as the `evaluation_rmse_gate` CTest.
  - `tools/benchmark`: latency profiles at 720p, 1080p, and 4K recorded in `docs/benchmarks.md` (stage-level micro-benchmarks tracked as follow-up work; the full-extractor profile covers the pipeline end-to-end).
  - `.github/workflows/ci.yml`: clang-format (pinned version), Ubuntu + macOS `-Werror` builds + ctest, clang-tidy, ASan/UBSan job, and a 100% line+branch coverage gate.
- **Exit Criteria**:
  - [x] RMSE $< 0.1$ pixels demonstrated on synthetic ground truth (`evaluation_rmse_gate` green).
  - [x] Latency profile documented in `docs/benchmarks.md` (incl. 4K).
  - [x] All CI checks passing green.

---

### Milestone 7: Geospatial Flood Perception Pack (OpenCV AI Competition 2026)
*Status: IN PROGRESS (Phases 0–2 Complete, Phase 3 Active)*

- **Target**: Glacier-collapse flood disaster analysis for Nepal (Lende Khola / Bhote Koshi, 2026-08-26 event) using Sentinel-2 L2A 10m imagery.
- **Key Deliverables**:
  - [x] Phase 0: Monorepo baseline validation (19/19 ctest, 0.0301 px RMSE gate, format check clean).
  - [x] Phase 1: Sentinel-2 image ingestion (PRE 2026-08-24, POST 2026-08-27, CTRL 2026-08-12), 100% valid-pixel alpha masks, gitignore binary isolation, metadata JSON.
  - [x] Phase 2: Sub-pixel ECC co-registration (0.154 px shift), 10-config scale $\times$ polarity sweep, weakly-supervised $F_1$ selection (union config $\sigma=1.5$), dual-polarity overlays, and side-by-side comparison panel (`comparison.png`).
  - [ ] Phase 3: `domains/geospatial` domain pack (`GeospatialFloodPack`), flood delta report (`flood_delta.json`), cloud masking, valid-area normalization, and channel width proxy.
  - [ ] Phase 4: Full CI/CD quality gate pass, `docs/flood_nepal_2026.md` with competition award mapping, and commits.
- **Exit Criteria**:
  - [x] Pre/post flood registration shift $< 3.0$ px.
  - [x] Weakly-supervised $F_1$ score evaluated across 10 configurations.
  - [ ] `flood_delta.json` demonstrates positive flood signature (`flood_fraction_ratio >= 1.10` or `width_ratio >= 1.25`).
  - [ ] Spatial and temporal control tests pass (`control_ratio < 1.10`).

---

## 3. Future Domain Expansion (Post-Milestone 7)

Once the core engine and geospatial flood pack are validated, specialized domain packs will be developed as isolated plug-ins:

1. **`domains/palm`**: Biometric hand landmark detection, palm ROI cropping, palm crease extraction, minutiae mapping, depth profiling. *(Implemented & Active)*
2. **`domains/surface_inspection`**: Micro-crack detection, paint anomaly profiling, scratch severity scoring for industrial QC. *(Implemented & Active)*
3. **`domains/geospatial`**: Flood damage mapping, sediment surge quantification, and river channel widening analysis. *(In Progress)*
4. **`domains/poultry`**: Flock tracking via ByteTrack adapter, density heatmap calculation, activity index, feeder queue monitoring.
5. **`domains/driver_monitoring`**: Eye aspect ratio (EAR), gaze vector, head orientation, drowsiness detection.
6. **`domains/document_scanning`**: 4-point perspective document boundary rectification, shadow suppression, and adaptive binarization.

