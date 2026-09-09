#pragma once

#include "curv/GeometryTypes.hpp"
#include "curv/geometry/RoiWarper.hpp"
#include <string>
#include <vector>

namespace CurvEngine::domain {

enum class DigitId { Thumb = 0, Index, Middle, Ring, Pinky };

struct DigitAnatomy {
    DigitId id{DigitId::Thumb};
    std::string name;
    std::vector<Point2D> joint_chain; // Joint chain from base to tip
    Point2D base{0.0f, 0.0f};         // MCP joint
    Point2D tip{0.0f, 0.0f};          // Fingertip
};

struct HandAnatomyResult {
    std::vector<DigitAnatomy> digits;       // 5 digit rays
    std::vector<Point2D> basal_mcp_crease;  // 2nd-level basal crease arc: Index MCP (P5) -> Pinky MCP (P17)
    std::vector<Point2D> thumb_crease;      // Thenar / Thumb boundary: P1 -> P2 -> P5
    std::vector<Point2D> central_palm_disc; // Closed polygon enclosing the primary palm body
    bool is_valid{false};
};

class HandAnatomyDecomposer {
public:
    /**
     * @brief Decomposes hand landmarks into 5 digit rays, 2nd-level basal crease dividers,
     * and isolates the central palm disc polygon.
     * @param landmarks 21-point MediaPipe hand landmarks.
     * @param image_width Frame width (or 1.0 if normalized).
     * @param image_height Frame height (or 1.0 if normalized).
     * @return HandAnatomyResult containing digits, basal arc, and palm disc.
     */
    [[nodiscard]] static HandAnatomyResult decompose(const geometry::HandLandmarks& landmarks, float image_width = 1.0f,
                                                     float image_height = 1.0f);

    /**
     * @brief Checks if a 2D point lies strictly inside the isolated central palm disc polygon.
     */
    [[nodiscard]] static bool isInsidePalmDisc(const Point2D& pt, const std::vector<Point2D>& palm_disc);
};

} // namespace CurvEngine::domain
