# Driver Gaze & Cabin Monitoring Pack (`org.curv.domain.driver_monitoring`)

> **Package**: `domains/driver_monitoring`  
> **Domain Pack Identifier**: `org.curv.domain.driver_monitoring`  
> **Architecture Mode**: Self-Sustained Monorepo Package (Directly Extractable)  
> **Application**: Automotive Driver Monitoring System (DMS), Commercial Fleet Safety, Euro NCAP Gaze Compliance

---

## 1. Package Purpose & Safety Standards

This package evaluates driver visual attention and vigilance from continuous gaze coordinate streams:
- **NHTSA 2.0s Threshold**: Detects glances off the forward roadway lasting $\ge 2.0\text{ seconds}$.
- **Attention Classification**: Identifies `alert_scanning`, `minor_distraction`, `critical_distraction`, and `drowsy_stare`.
- **AOI Scanpath Encoding**: Symbolic tokenization of gaze across windshield, mirrors, and instrument cluster.

---

## 2. Standalone Package Documentation Index

This package is self-sustained and directly extractable. All architectural models and safety rules are maintained locally:

| Document | File Path | Scope & Focus |
| :--- | :--- | :--- |
| **System Architecture** | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Component flow, in-cabin AOI layout, NHTSA distraction heuristics, and state definitions. |

---

## 3. Package Structure & Extraction Layout

```text
domains/driver_monitoring/
├── README.md               # Package manifest and documentation index
├── docs/
│   └── ARCHITECTURE.md     # Architectural specification
├── include/
│   └── curv/domains/
│       └── DriverGazePack.hpp   # Public C++20 domain pack contract
└── src/
    └── DriverGazePack.cpp       # Zero-dependency domain pack implementation
```

---

## 4. C++ API Usage

```cpp
#include "curv/domains/DriverGazePack.hpp"

using namespace CurvEngine::temporal;
using namespace CurvEngine::domains;

// 1. Define In-Cabin Areas of Interest
AreaOfInterest windshield{"Windshield", 200.0, 100.0, 800.0, 600.0};
AreaOfInterest phone_mount{"PhoneMount", 50.0, 650.0, 180.0, 800.0};
AreaOfInterest left_mirror{"LeftMirror", 0.0, 150.0, 150.0, 350.0};

// 2. Ingest stream of GazeSample items from camera DMS
std::vector<GazeSample> samples = /* DMS gaze telemetry */;

// 3. Evaluate visual attention & distraction
DriverAttentionReport report =
    DriverGazePack::evaluateVisualAttention(samples, windshield, {phone_mount, left_mirror});

if (report.attention_state == "critical_distraction") {
    // Trigger in-cabin acoustic alert or haptic steering warning
}
```
