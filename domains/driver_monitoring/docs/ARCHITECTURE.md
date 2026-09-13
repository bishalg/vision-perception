# Architecture Specification: Driver Gaze & In-Cabin Visual Attention

> **Package**: `domains/driver_monitoring`  
> **Domain Identifier**: `org.curv.domain.driver_monitoring`  
> **Extraction Readiness**: Standalone Package / Module  
> **Standards Alignment**: NHTSA Visual Distraction Guidelines, Euro NCAP Driver Engagement Criteria

---

## 1. Safety Principles & In-Cabin Visual Attention

Visual distraction is a leading cause of highway accidents. This package translates 2D/3D driver eye gaze telemetry and head pose vectors into quantitative alertness and distraction metrics:

### 1.1 The NHTSA 2.0-Second Off-Road Rule
National Highway Traffic Safety Administration (NHTSA) visual-manual driver distraction guidelines establish that glances away from the forward roadway lasting **$\ge 2.0\text{ seconds}$** significantly elevate collision risk.
- The `DriverGazePack` flags any single off-road fixation with duration $\ge 2000\text{ ms}$ as a `critical_off_road_glance`.

### 1.2 In-Cabin Areas of Interest (AOIs)
```text
┌─────────────────────────────────────────────────────────┐
│ [Left Mirror]    [Forward Windshield AOI]   [Right Mirror]│
│                     (Primary Roadway)                   │
├─────────────────────────────────────────────────────────┤
│  [Cluster]            [Infotainment]       [Phone Mount]│
└─────────────────────────────────────────────────────────┘
```

---

## 2. Package Structure & Extraction Layout

```text
domains/driver_monitoring/
├── README.md               # Package manifest and documentation index
├── docs/
│   └── ARCHITECTURE.md     # This architectural specification
├── include/
│   └── curv/domains/
│       └── DriverGazePack.hpp   # Public C++20 domain pack contract
└── src/
    └── DriverGazePack.cpp       # Zero-dependency domain pack implementation
```

---

## 3. Attention States & Classification Heuristics

The engine classifies driver attention into four discrete clinical/operational states:

| Attention State | Mathematical Condition | Behavioral Interpretation |
| :--- | :--- | :--- |
| **`alert_scanning`** | Off-road dwell $< 20\%$ and zero critical glances | Active visual scanning across forward roadway and mirrors. |
| **`minor_distraction`** | $20\% \le \text{Off-road dwell} < 40\%$ and zero glances $\ge 2.0\text{s}$ | Brief checks to mirrors, instrument cluster, or climate controls. |
| **`critical_distraction`** | $\ge 1$ glance $\ge 2.0\text{s}$ OR Off-road dwell $\ge 40\%$ | Dangerous gaze diversion (e.g. texting, browsing infotainment). |
| **`drowsy_stare`** | Mean fixation duration $\ge 1500\text{ ms}$ with low saccadic rate | Tunnel vision, cognitive underload, or micro-sleep onset. |
