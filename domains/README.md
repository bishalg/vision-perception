# Domain Extension Packs

Domain packs contain high-level, application-specific interpretation logic that builds upon the generic perception outputs of `core/`.

## Architectural Invariant
The `core/` engine contains **zero domain logic**. All domain-specific semantics, anatomical references, defect rules, and behavioral models live strictly in this directory as modular plugins.

## Domain Packs Status

### Implemented & Active
- **`palm/`** (`org.curv.domain.palm`): Biometric hand landmark alignment (21 keypoints), handedness detection, canonical 512x512 upright palm warp, adaptive Steger parameter scaling, crease classification, and grounded VLM prompt generation (Vedic Samudrika Shastra).
- **`surface_inspection/`** (`org.curv.domain.surface_inspection`): Industrial quality control defect analysis, quad-ROI perspective normalization with coordinate re-inversion, micro-crack/scratch classification, and severity scoring.
- **`geospatial/`** (`org.curv.domain.geospatial`): Satellite-based hydrological corridor and glacier-collapse flood damage perception (Sentinel-2 L2A 10m), river trunk vs tributary classification, and multi-temporal flood delta quantification.

### Planned (Post-M7 Domain Expansion)
- **`poultry/`**: Flock tracking via ByteTrack adapter, density heatmap calculation, movement velocity, and behavioral dwell-time analysis.
- **`driver_monitoring/`**: In-cabin safety, eye aspect ratio (EAR), gaze vector estimation, and driver distraction/drowsiness scoring.
- **`document_scanning/`**: 4-point perspective boundary rectification, shadow suppression, and adaptive stroke binarization.


