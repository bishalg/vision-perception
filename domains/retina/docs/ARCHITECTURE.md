# Architecture Specification: Retinal Micro-Vascular Perception

> **Package**: `domains/retina`  
> **Domain Identifier**: `org.curv.domain.retina`  
> **Extraction Readiness**: Standalone Package / Module  
> **Applicable Platforms**: Desktop (C++20 / OpenCV), iOS / iPadOS (`libcurv_capi.xcframework`), Android JNI

---

## 1. Mathematical & Optical Overview

Retinal blood vessel morphology is an established, non-invasive diagnostic window into systemic micro-circulatory health (diabetic retinopathy, hypertensive vascular remodeling, stroke risk, and cardiovascular disease).

### Optical Channel Selection
- Hemoglobin demonstrates an optical absorption peak in the green spectrum ($\approx 540\text{--}575\text{ nm}$).
- Color fundus photography ($RGB$) inherently suffers from low vessel contrast in red and blue channels.
- `prepareGreenChannel(bgr)` isolates the green channel, inverts intensity so vessels become bright curvilinear ridges, and applies Contrast-Limited Adaptive Histogram Equalization (CLAHE) to normalize non-uniform illumination across the retinal fundus.

```mermaid
flowchart LR
    A[RGB Fundus Photo] --> B[Green Channel Isolation]
    B --> C[Intensity Inversion]
    C --> D[CLAHE Contrast Equalization]
    D --> E[Scale-Space Steger Ridge Extraction]
    E --> F[Topological Junction Analysis]
    F --> G[Clinical Biomarker Extraction]
```

---

## 2. Package Architecture & Layout

```text
domains/retina/
├── README.md               # Package manifest and documentation index
├── docs/
│   ├── ARCHITECTURE.md     # This architectural specification
│   └── CLINICAL_BIOMARKERS.md # Mathematical definitions of tortuosity, VDR, and caliber
├── include/
│   └── curv/domains/
│       └── RetinaDomainPack.hpp   # C++20 IDomainPack implementation
└── src/
    └── RetinaDomainPack.cpp       # Zero-dependency domain pack implementation
```

---

## 3. Core Algorithm Flow

1. **Preprocessing**:
   - `prepareGreenChannel(bgr, apply_clahe = true)`: Converts 3-channel fundus into enhanced, inverted vessel ridge map.
2. **Curvilinear Ridge Detection**:
   - Runs `curv::ridge::StegerRidgeExtractor` with scale parameter $\sigma \in [1.0, 2.5]$.
   - Hessian eigenvalue decomposition locates sub-pixel centerlines with $< 0.1\text{ px}$ error.
3. **Domain Processing**:
   - Computes arc-chord tortuosity ($\tau = L_{\text{arc}} / L_{\text{chord}}$) for each vessel segment.
   - Categorizes segments into **Primary Arcades**, **Secondary Branches**, and **Micro-Capillaries**.
   - Identifies bifurcations from topological junctions with degree $\ge 3$.
   - Computes the **Vessel Density Ratio (VDR)** over the visible field.
