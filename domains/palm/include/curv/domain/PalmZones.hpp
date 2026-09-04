#pragma once

#include <vector>
#include <string>
#include "curv/geometry/ZoneIntersector.hpp"

namespace CurvEngine::domains {

/**
 * @brief Palm domain anatomical zones defined in canonical 512x512 upright patch space.
 * 
 * Orientations in canonical 512x512 patch:
 * - Upward (-Y): Digits (Index, Middle, Ring, Pinky)
 * - Downward (+Y): Wrist and Carpal creases
 * - Left (-X): Radial side (Thumb, Index)
 * - Right (+X): Ulnar side (Pinky, Percussion)
 */
class PalmZones {
public:
    /**
     * @brief Retrieves canonical palm zones for 512x512 upright space.
     * @param target_dim Patch dimension (default: 512).
     * @param is_left_hand When true, mirrors zones horizontally to match a left palm facing the camera.
     * @return List of polygonal anatomical zones.
     */
    static std::vector<geometry::Zone> getCanonicalPalmZones(int target_dim = 512, bool is_left_hand = false);
};

} // namespace CurvEngine::domains

namespace CurvEngine::domain {
    using PalmZones = CurvEngine::domains::PalmZones;
}
