#pragma once

#include <string>
#include <vector>

#include "curv/GeometryTypes.hpp"
#include "curv/geometry/ZoneIntersector.hpp"
#include "curv/pattern/MicroPatternDetector.hpp"

namespace CurvEngine::domain {

// Mount-mark detection for the two specified FateLineCases rows
// (bad_saturn_mount, bad_mercury_mount): grill/cross/star figures sitting on
// the Saturn or Mercury mount zones.
//
// Mechanism (honest proxy, documented): a detected Cross or Star pattern whose
// centroid falls inside the mount polygon (plus Island figures where the spec
// row lists them), OR a junction-density cluster (>= kGrillJunctionMin
// junctions — grill ≈ grid of short ridges) in the zone.
// Star-only figures without cross/junction support remain future work; the
// table rows' classical `use` text is unchanged by this wiring.
struct MountMarks {
    bool saturn_bad{false};
    bool mercury_bad{false};
};

class MountMarkDetector {
public:
    // Placeholder density gates until data-calibrated (see LineLengthThresholds
    // honesty contract; same rule applies here).
    static constexpr int kGrillJunctionMin = 5;
    static constexpr int kHorizSegmentMin = 2;
    static constexpr float kHorizMaxLengthPx = 20.0F;

    [[nodiscard]] static MountMarks detect(const RidgeGraph& graph, const std::vector<geometry::Zone>& zones);
};

} // namespace CurvEngine::domain

namespace CurvEngine::domains {
using MountMarks = CurvEngine::domain::MountMarks;
using MountMarkDetector = CurvEngine::domain::MountMarkDetector;
} // namespace CurvEngine::domains
