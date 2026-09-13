# Retinal Vessel Domain Pack (`org.curv.domain.retina`)

> **Package**: `domains/retina`  
> **Domain Pack Identifier**: `org.curv.domain.retina`  
> **Architecture Mode**: Self-Sustained Monorepo Package (Directly Extractable)  
> **Application**: Quantitative Ophthalmology, Micro-Vascular Biomarker Analysis, Stroke / Cardiovascular Risk

---

## 1. Package Purpose & Clinical Overview

This package interprets sub-pixel curvilinear ridges extracted from retinal fundus photographs and computes quantitative vascular biomarkers:
- **Green Channel Contrast**: Isolates hemoglobin absorption band ($\approx 540\text{--}575\text{ nm}$) with CLAHE equalization.
- **Arc-Chord Tortuosity ($\tau$)**: Quantifies vessel curvature and arterial tortuosity.
- **Vessel Density Ratio (VDR)**: Measures micro-vascular perfusion area.
- **Topological Bifurcations**: Detects branching nodes from junction graphs with degree $\ge 3$.

---

## 2. Standalone Package Documentation Index

This package is self-sustained and directly extractable. All architectural models and metric definitions are maintained locally:

| Document | File Path | Scope & Focus |
| :--- | :--- | :--- |
| **System Architecture** | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Component flow, green-channel extraction, Steger ridge integration, and module layout. |
| **Clinical Biomarkers** | [`docs/CLINICAL_BIOMARKERS.md`](docs/CLINICAL_BIOMARKERS.md) | Mathematical formulas for arc-chord tortuosity, VDR, and vessel caliber classification. |

---

## 3. Package Structure & Extraction Layout

```text
domains/retina/
├── README.md               # Package manifest and documentation index
├── docs/                   # Self-sustained package specifications
│   ├── ARCHITECTURE.md     # Architectural specification
│   └── CLINICAL_BIOMARKERS.md # Mathematical definitions of clinical biomarkers
├── include/
│   └── curv/domains/
│       └── RetinaDomainPack.hpp   # Public C++20 domain pack contract
└── src/
    └── RetinaDomainPack.cpp       # Zero-dependency implementation
```

---

## 4. C++ API Usage

```cpp
#include "curv/domains/RetinaDomainPack.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"
#include <opencv2/imgcodecs.hpp>

using namespace CurvEngine;
using namespace CurvEngine::domains;

// 1. Ingest color fundus photograph
cv::Mat fundus_bgr = cv::imread("path/to/fundus.png");

// 2. Preprocess: isolate green channel, invert, and apply CLAHE
cv::Mat prepped = RetinaDomainPack::prepareGreenChannel(fundus_bgr, true);

// 3. Extract sub-pixel curvilinear vessel ridges
ridge::StegerConfig cfg;
cfg.sigma = 1.5f;
ridge::StegerRidgeExtractor extractor(cfg);
RidgeGraph ridges = extractor.extractRidgeGraph(prepped);

// 4. Process domain evidence and biomarkers
Frame frame(prepped, "fundus_sample", 1000);
QualityReport quality(0.05, 0.55, 0.85, true, "Optimal");
RetinaDomainPack pack(10.0f);

DomainInput input{frame, ridges, quality, nullptr};
DomainOutput output = pack.process(input);

// Access results
double vdr = output.domain_payload["vessel_density_ratio"];
double tortuosity = output.domain_payload["mean_tortuosity_index"];
int bifurcations = output.domain_payload["bifurcation_count"];
```
