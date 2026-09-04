# Domain Pack Specification & Plugin Architecture

> **Specification Version**: 1.0.0  
> **Status**: Approved Domain Extension Standard  
> **Namespace**: `vision::domains`

---

## 1. Concept & Scope

A **Domain Pack** is an isolated, pluggable software component that interprets the domain-agnostic perception outputs produced by the `vision-perception` core engine.

### Strict Architectural Boundaries
- **Core Stays Agnostic**: The `core/` engine has zero knowledge of any domain pack. It does not import domain headers, mention domain concepts, or include domain-specific heuristics.
- **Unidirectional Dependency**: Domain packs depend on `core/include/` (and optionally `adapters/`), never vice versa.
- **Pluggable Registration**: Domain packs implement standard lifecycle and execution interfaces, allowing them to be dynamically loaded or statically linked.

---

## 2. The Domain Pack Interface Contract

Every domain pack must implement the pure virtual `IDomainPack` interface:

```cpp
#pragma once

#include <memory>
#include <string>
#include <opencv2/core.hpp>
#include "curv/GeometryTypes.hpp"

namespace vision::domains {

struct DomainConfig {
    std::string config_json;
    float confidence_threshold{0.5f};
    bool enable_telemetry{true};
};

struct DomainResult {
    bool success{false};
    std::string domain_name;
    std::string result_payload_json;
    cv::Mat visual_debug_overlay;
};

class IDomainPack {
public:
    virtual ~IDomainPack() = default;

    [[nodiscard]] virtual std::string name() const noexcept = 0;
    [[nodiscard]] virtual std::string version() const noexcept = 0;

    virtual bool initialize(const DomainConfig& config) = 0;

    [[nodiscard]] virtual DomainResult process(
        const CurvEngine::RidgeGraph& ridge_graph,
        const cv::Mat& source_frame) = 0;

    virtual void teardown() noexcept = 0;
};

} // namespace vision::domains
```

---

## 3. Standard Domain Pack Descriptions

### 3.1. `domains/palm/`
- **Objective**: Biometric hand analysis and structural palm crease mapping.
- **Input**: Extracted `RidgeGraph` and hand bounding frame.
- **Domain Tasks**: Landmark anchoring, major crease classification (Heart, Head, Life, Fate lines), crease depth profiling, minutiae detection.
- **Prohibited in Core**: Astrological interpretations, palmistry terminology, or religious divination rules.

### 3.2. `domains/poultry/`
- **Objective**: Livestock tracking, density estimation, and flock behavior analytics.
- **Input**: Sequential video frames, background subtraction masks, and curvilinear motion contours.
- **Domain Tasks**: Individual bird segmentation, centroid tracking, velocity vector mapping, feeding dwell-time histograms.

### 3.3. `domains/driver_monitoring/`
- **Objective**: In-cabin safety, driver attentiveness, and distraction assessment.
- **Input**: Facial frame region of interest and extracted facial crease/edge geometries.
- **Domain Tasks**: Eye Aspect Ratio (EAR), blink frequency, gaze direction estimation, head pose orientation.

### 3.4. `domains/surface_inspection/`
- **Objective**: Industrial quality control for flat surfaces (metals, glass, solar wafers, textiles).
- **Input**: High-resolution grayscale or polarized surface scans.
- **Domain Tasks**: Micro-crack identification, scratch length and depth estimation, defect clustering, pass/fail grading.

### 3.5. `domains/document_scanning/`
- **Objective**: Document edge detection, boundary rectification, and text ridge extraction.
- **Input**: Raw document photograph or scan.
- **Domain Tasks**: 4-point convex hull extraction, homography warping, shadow suppression, stroke thickness analysis.
