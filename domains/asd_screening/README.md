# ASD Social Attention Biomarker Pack (`org.curv.domain.asd_screening`)

> **Package**: `domains/asd_screening`  
> **Domain Pack Identifier**: `org.curv.domain.asd_screening`  
> **Architecture Mode**: Self-Sustained Monorepo Package (Directly Extractable)  
> **Target Cohort**: Toddlers (18–48 months) accompanied by caregiver  
> **Clinical Benchmarks**: Duke University *SenseToKnow*, FDA-cleared *EarliPoint*

---

## 1. Package Purpose & Clinical Scoping

This package provides an offline-first, pediatric social visual attention and oculomotor biomarker evaluation engine. It translates 60 Hz mobile gaze telemetry from TrueDepth cameras into quantitative behavioral phenotypes.

> [!CAUTION]
> This domain pack is a **clinical research and screening support tool**. It does not provide automated diagnoses of Autism Spectrum Disorder (ASD). All outputs represent continuous statistical distributions designed to assist pediatric specialists.

---

## 2. Standalone Package Documentation Index

This domain pack is fully self-sustained. Its specifications, protocols, and architectural models are maintained within this folder for modularity and seamless standalone repository extraction:

| Document | File Path | Scope & Focus |
| :--- | :--- | :--- |
| **System Architecture** | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Component hierarchy, offline iOS data flow, TrueDepth raycasting, and quality gates. |
| **iOS Native App Design & Tracker** | [`docs/IOS_APP_DESIGN_AND_ROADMAP.md`](docs/IOS_APP_DESIGN_AND_ROADMAP.md) | 7-phase iOS implementation roadmap, TrueDepth raycaster, AVPlayer sync, and ResearchKit/CareKit guide. |
| **Data Collection Schema** | [`docs/DATA_COLLECTION_SCHEMA.md`](docs/DATA_COLLECTION_SCHEMA.md) | Complete 83-field per-frame schema: gaze, head pose, 52 FACS affect metrics, AOI, stimulus sync, quality. |
| **Data Storage & Pipeline** | [`docs/DATA_STORAGE_AND_PIPELINE.md`](docs/DATA_STORAGE_AND_PIPELINE.md) | AES-GCM-256 encrypted vault, session JSONL format, 5-phase pipeline roadmap (D1–D5), export contracts. |
| **Stimuli & Dynamic AOIs** | [`docs/STIMULI_SPEC.md`](docs/STIMULI_SPEC.md) | Specifications for GeoPref, Face Scanning, and Joint Attention tasks, with dynamic AOIs. |
| **Ethics & Consent Protocol** | [`docs/ETHICS_PROTOCOL.md`](docs/ETHICS_PROTOCOL.md) | IRB protocol, COPPA/HIPAA protections, zero raw video storage, and 5-point bubble calibration. |
| **ML & Validation Roadmap** | [`docs/ML_ROADMAP.md`](docs/ML_ROADMAP.md) | Grouped cross-validation (by `child_id`), 16 explainable biomarkers, SHAP, and CoreML export. |
| **Literature & 12-Phase SaMD** | [`docs/LITERATURE.md`](docs/LITERATURE.md) | Clinical literature review (Klin, Jones, Pierce, Sapiro) and 12-phase SaMD development lifecycle. |

---

## 3. Package Structure & Extraction Layout

This folder is structured to allow immediate standalone extraction (e.g. `git subtree split` or extraction into a dedicated iOS framework):

```text
domains/asd_screening/
├── README.md                           # Package manifest and documentation index
├── docs/                               # Complete standalone specifications
│   ├── ARCHITECTURE.md                 # Architectural specification
│   ├── IOS_APP_DESIGN_AND_ROADMAP.md   # Native iOS app design & phase tracking
│   ├── STIMULI_SPEC.md                 # Stimuli timing, resolution, and AOI schemas
│   ├── ETHICS_PROTOCOL.md              # Privacy, IRB, and calibration specifications
│   ├── ML_ROADMAP.md                   # Validation protocols and CoreML export
│   └── LITERATURE.md                   # Scientific foundations & 12-phase roadmap
├── include/
│   └── curv/domains/
│       └── AsdBiomarkerPack.hpp        # C++20 API & Apple Health data contracts
├── src/
│   └── AsdBiomarkerPack.cpp            # Zero-dependency implementation
└── ios/                                # Reusable Swift 6 sensor package (CurvAsdGazeKit)
    ├── Package.swift
    ├── Sources/CurvAsdGazeKit/
    └── Tests/CurvAsdGazeKitTests/
```

---

## 4. C++ API & Apple Health Ecosystem Integration

### 4.1 Evaluating a Gaze Session
```cpp
#include "curv/domains/AsdBiomarkerPack.hpp"

using namespace CurvEngine::temporal;
using namespace CurvEngine::domains;

// 1. Define Areas of Interest (e.g. Split-Screen GeoPref)
std::vector<AreaOfInterest> aois = {
    {"Social_Actor", 0.0, 0.0, 500.0, 800.0},
    {"Geometric_Pattern", 500.0, 0.0, 1000.0, 800.0}
};

// 2. Ingest stream of GazeSample items from ARKit
std::vector<GazeSample> samples = /* 60Hz gaze telemetry */;

// 3. Evaluate clinical biomarkers & quality
AsdSessionConfig config;
config.min_valid_ratio = 0.60;

AsdSessionReport report = AsdBiomarkerPack::evaluateSession(samples, aois, config);

if (report.is_valid) {
    double social_pref = report.biomarkers.social_preference_score; // [-1.0, +1.0]
    double eye_dwell = report.biomarkers.eye_dwell_ratio;
    
    // 4. Export to Apple HealthKit / ResearchKit JSON
    nlohmann::json rk_json = report.toResearchKitJson();
    nlohmann::json ck_json = report.toCareKitJson();
}
```

### 4.2 ResearchKit JSON Contract (`ORKActiveStepResult`)
```json
{
  "identifier": "ORKActiveStepResult",
  "type": "social_attention_gaze_result",
  "is_valid": true,
  "valid_sample_ratio": 0.94,
  "quality_verdict": "optimal",
  "biomarkers": {
    "social_preference_score": 0.48,
    "eye_dwell_ratio": 0.28,
    "mouth_dwell_ratio": 0.18,
    "face_dwell_ratio": 0.55,
    "first_eye_latency_ms": 620.0,
    "mean_fixation_duration_ms": 295.0,
    "transition_entropy": 1.78,
    "repeat_bigram_ratio": 0.25,
    "recurrence_rate": 0.19
  }
}
```

### 4.3 CareKit JSON Contract (`OCKCarePlanActivity`)
```json
{
  "care_plan_element": "OCKCarePlanActivity",
  "task_title": "Social Attention Assessment",
  "completion_status": "completed",
  "summary_metrics": {
    "social_preference_index": 0.48,
    "eye_contact_ratio": 0.28,
    "fixation_stability_ms": 295.0
  }
}
```

---

## 5. Swift Sensor Kit (`CurvAsdGazeKit`, `ios/`)

Reusable Swift 6 package linked by `apps/ios_asd_gaze_app`. TrueDepth
`ARFaceAnchor` 3D face-mesh tracking → 60 Hz numerical gaze telemetry; zero
raw video is ever stored.

| Unit | Role |
| :--- | :--- |
| `GazeSessionController` | ARSession ownership, per-frame face-anchor ingest, head-pose + distance |
| `ScreenRaycaster` | Face/eye 3D transforms + `lookAtPoint` → screen-point gaze projection |
| `GazeProcessingActor` | OneEuro filtering, blink rejection, fixation/saccade (I-VT) pipeline |
| `GazeCalibrationProfile` | 5-point bubble calibration mapping |
| `StimulusAOITracker` | Dynamic AOI dwell, GeoPref split-screen scoring |
| `FacialAffectMetrics` | 52-field FACS blendshape affect vector |
| `ProceduralOculomotorStimulusView` | Code-drawn pursuit/saccade targets (no video files) |
| `BiomarkerClassificationEngine` | On-device biomarker scoring → risk stratum |
| `SessionVaultManager` / `GazeQualityGate` | AES-GCM-256 session vault, validity gating |

61 unit tests (`ios/Tests/`); app-level suites (playlist integrity, CareKit,
ResearchKit sync, report generation) live in `apps/ios_asd_gaze_app/Tests/`
(55 tests). App features and screening flow:
[`apps/ios_asd_gaze_app/README.md`](../../apps/ios_asd_gaze_app/README.md).

## Contract, synthetic data and norms tooling (Rust)

The session JSON Schema, the synthetic session generator and the normative-baseline aggregator
are `asd-cli` commands (the former Python scripts were removed):

```bash
cargo run -p asd-cli -- schema check                      # stored schemas match the Rust contract
cargo run -p asd-cli -- schema validate docs/spec/schemas/asd/session-v1.6.json <file>...
cargo run -p asd-cli -- synth --seed 7 --count 50 --out ./synthetic_sessions
cargo run -p asd-cli -- norms aggregate --input <dir> --output <file> [--min-n 12]
```

Run from `rust/`. Tests live in `rust/crates/asd-core/tests/` (`synth.rs`, `norms.rs`).
