# Domain Extension Packs

Domain packs contain high-level, application-specific interpretation logic that builds upon the generic perception outputs of `core/`.

## Architectural Invariant
The `core/` engine contains **zero domain logic**. All domain-specific semantics, anatomical references, defect rules, and behavioral models live strictly in this directory as modular plugins.

## Domain Directory Index
- **`palm/`**: Biometric hand ROI extraction, palm crease mapping (major and minor lines), crease depth profiling, and minutiae detection.
- **`poultry/`**: Flock tracking, spatial clustering, movement velocity, and behavioral dwell-time analysis.
- **`driver_monitoring/`**: In-cabin safety, eye aspect ratio (EAR), gaze vector estimation, and driver distraction scoring.
- **`surface_inspection/`**: Industrial automated defect analysis, micro-crack tracking, and surface scratch profiling.
- **`document_scanning/`**: 4-point perspective boundary rectification, shadow suppression, and adaptive stroke binarization.
