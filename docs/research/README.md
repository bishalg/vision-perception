# Research & Technical Discovery Index

> **Directory**: `docs/research/`  
> **Status**: Active Architectural Specifications & Field Findings  
> **Format**: High-density, token-efficient index optimized for fast human review and AI agents.

---

## 1. Document Directory

| Document | Identifier | Focus Area | Key Findings / Technologies |
| :--- | :--- | :--- | :--- |
| **[`gods_eye_view_and_cctv_integration.md`](gods_eye_view_and_cctv_integration.md)** | `DOC-RES-004` | CCTV & Public Spatial Intelligence | • Open CCTV endpoints (Caltrans, Austin, TfL).<br>• 3D Camera pose & viewshed frustum projection.<br>• FLIR/Ironbow & NVG GLSL shader simulation.<br>• Real-time ADS-B, AIS, and SGP4 orbital feeds. |
| **[`realtime_spatial_perception_architecture.md`](realtime_spatial_perception_architecture.md)** | `DOC-ARCH-005` | System-Wide Real-Time Ingestion | • Multi-tier architecture: Ingestion $\to$ Core $\to$ Cloud $\to$ 3D Twin.<br>• Thread-safe ring-buffer zero-latency frame ingestion.<br>• 4-Phase implementation roadmap (CCTV $\to$ SAR $\to$ Viewshed $\to$ Twin). |
| **[`github_reuse_matrix.md`](github_reuse_matrix.md)** | `DOC-RES-001` | Open-Source Component Audit | • License audit (Apache-2.0, MIT, BSD vs GPL rejection).<br>• OpenCV core/imgproc, Steger reference, spline libraries. |
| **[`palm_next_phase.md`](palm_next_phase.md)** | `DOC-RES-006` | Palm app next phase (analysis, not detector) | • Do not rebuild Steger / U-Net / Canny.<br>• Borrow feature lists from GitHub palmistry, not their code.<br>• `palm_analysis` + `user_report` contract; shrink VLM prompt.<br>• Phases P0–P3: wire linker → geometry → user report → mobile. |

---

## 2. Related Core Specifications

- **[`docs/flood_realtime_architecture.md`](../flood_realtime_architecture.md)**: Production real-time glacier-collapse and flash flood monitoring specification (Sentinel-1 SAR, Sentinel-2 L2A, river CCTV, DHM hydrometeorology, AWS Bedrock dispatch).
- **[`docs/flood_nepal_2026.md`](../flood_nepal_2026.md)**: Empirical validation of the Lende Khola / Bhote Koshi disaster in Nepal (+88.4% flood surge, +121.1% channel widening, dual controls passing).
- **[`docs/architecture.md`](../architecture.md)**: Domain-agnostic C++20 core rules, MVC boundaries, and zero-domain-pollution invariants.
