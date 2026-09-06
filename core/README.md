# CurvEngine Core (Domain-Agnostic Perception Engine)

`curv_core` is the technology-independent heart of the platform: sub-pixel curvilinear
ridge extraction, frame quality gating, ROI normalization, and evidence assembly.
It knows nothing about palms, cracks, or any application domain — that lives in
`domains/` (see the architectural invariant in the root README: no domain pollution).

## Layout

- `include/curv/GeometryTypes.hpp` — the data contract: `Point2D`, `CurveSegment`, `Junction`, `RidgeGraph`.
- `include/curv/ridge/` — Steger (1998) sub-pixel ridge/valley extraction: Gaussian scale-space,
  closed-form 2x2 Hessian eigendecomposition (`HessianMath.hpp`), directional Taylor zero-crossing,
  hysteresis linking (`StegerRidgeExtractor.hpp`), parallelized with `cv::parallel_for_`.
- `include/curv/quality/` — Laplacian-variance focus/blur quality gate (`IQualityAnalyzer` interface).
- `include/curv/pipeline/SyncPerceptionPipeline.hpp` — orchestrator: quality gate → ridge extraction →
  `PipelineResult{is_usable, QualityReport, RidgeGraph, Evidence[], total_duration_us}`.
- `include/curv/geometry/` — quad-ROI perspective warp (`RoiWarper`), zone polygon intersection (`ZoneIntersector`).
- `include/curv/roi/CanonicalWarper.hpp` — canonical 512x512 upright palm patch warp from 21 landmarks.
- `include/curv/config/DynamicParameterResolver.hpp` — auto-tunes `StegerConfig` from ROI pixel diagonal (scale invariance).
- `include/curv/interfaces/` — `IDomainPack`, `ILineCurveExtractor`, `IPreprocessor`, `IQualityAnalyzer`.
- `src/` — implementation units for everything above.

Consumed by `adapters/`, `domains/`, and every tool; built as the `curv_core` static target
(alias `vision_perception_core`). Tests live in the top-level `tests/` suite.

## Milestone Status & Verification

- **Current Version**: `0.1.0` (C++20, `-Werror`, `-Wall -Wextra -Wpedantic -Wconversion`)
- **Milestones 1–4 Completed**:
  - Unbiased Steger differential-geometry ridge extraction engine (`HessianMath`, `StegerRidgeExtractor`)
  - Laplacian-variance real-time focus & blur quality gate (`LaplacianQualityAnalyzer`)
  - Perspective normalization & sub-pixel coordinate inversion (`RoiWarper`, `CanonicalWarper`)
  - Scale-space parameter auto-tuning (`DynamicParameterResolver`)
  - Immutable evidence model and structured pipeline orchestrator (`SyncPerceptionPipeline`)
- **Verification**: Verified under GoogleTest suite, ASan+UBSan, and llvm-cov source-based coverage.

