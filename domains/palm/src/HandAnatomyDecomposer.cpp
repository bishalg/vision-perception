#include "curv/domain/HandAnatomyDecomposer.hpp"
#include <cmath>

namespace CurvEngine::domain {

HandAnatomyResult HandAnatomyDecomposer::decompose(const geometry::HandLandmarks& landmarks, float image_width,
                                                   float image_height) {
    HandAnatomyResult res;

    // 1. Validation
    float sum = 0.0f;
    for (const auto& pt : landmarks.points) {
        sum += std::abs(pt.x) + std::abs(pt.y);
    }
    if (sum < 1e-4f) {
        return res;
    }

    const float sx = landmarks.is_normalized ? image_width : 1.0f;
    const float sy = landmarks.is_normalized ? image_height : 1.0f;

    auto to_pt = [&](int idx) -> Point2D {
        return Point2D(landmarks.points[static_cast<size_t>(idx)].x * sx,
                       landmarks.points[static_cast<size_t>(idx)].y * sy);
    };

    // 2. 5 Digits Decomposition
    // Thumb: P1 (CMC), P2 (MCP), P3 (IP), P4 (TIP)
    DigitAnatomy thumb;
    thumb.id = DigitId::Thumb;
    thumb.name = "Thumb";
    thumb.joint_chain = {to_pt(1), to_pt(2), to_pt(3), to_pt(4)};
    thumb.base = to_pt(2);
    thumb.tip = to_pt(4);
    res.digits.push_back(thumb);

    // Index: P5 (MCP), P6 (PIP), P7 (DIP), P8 (TIP)
    DigitAnatomy index;
    index.id = DigitId::Index;
    index.name = "Index";
    index.joint_chain = {to_pt(5), to_pt(6), to_pt(7), to_pt(8)};
    index.base = to_pt(5);
    index.tip = to_pt(8);
    res.digits.push_back(index);

    // Middle: P9 (MCP), P10 (PIP), P11 (DIP), P12 (TIP)
    DigitAnatomy middle;
    middle.id = DigitId::Middle;
    middle.name = "Middle";
    middle.joint_chain = {to_pt(9), to_pt(10), to_pt(11), to_pt(12)};
    middle.base = to_pt(9);
    middle.tip = to_pt(12);
    res.digits.push_back(middle);

    // Ring: P13 (MCP), P14 (PIP), P15 (DIP), P16 (TIP)
    DigitAnatomy ring;
    ring.id = DigitId::Ring;
    ring.name = "Ring";
    ring.joint_chain = {to_pt(13), to_pt(14), to_pt(15), to_pt(16)};
    ring.base = to_pt(13);
    ring.tip = to_pt(16);
    res.digits.push_back(ring);

    // Pinky: P17 (MCP), P18 (PIP), P19 (DIP), P20 (TIP)
    DigitAnatomy pinky;
    pinky.id = DigitId::Pinky;
    pinky.name = "Pinky";
    pinky.joint_chain = {to_pt(17), to_pt(18), to_pt(19), to_pt(20)};
    pinky.base = to_pt(17);
    pinky.tip = to_pt(20);
    res.digits.push_back(pinky);

    // 3. Basal MCP Crease Arc (2nd-Level Differentiation)
    // Runs transversely across the base of the four fingers from Index MCP to Pinky MCP
    res.basal_mcp_crease = {to_pt(5), to_pt(9), to_pt(13), to_pt(17)};

    // 4. Thenar / Thumb Boundary Crease
    res.thumb_crease = {to_pt(1), to_pt(2), to_pt(5)};

    // 5. Central Palm Disc Isolation Polygon
    // Traces: P5 -> P9 -> P13 -> P17 -> Hypothenar flank -> P0 (Wrist) -> Thenar flank -> P1 -> P2 -> P5
    const Point2D p0 = to_pt(0);
    const Point2D p1 = to_pt(1);
    const Point2D p2 = to_pt(2);
    const Point2D p5 = to_pt(5);
    const Point2D p9 = to_pt(9);
    const Point2D p13 = to_pt(13);
    const Point2D p17 = to_pt(17);

    // Interpolated hypothenar margin point between P17 and P0
    const Point2D p_hypothenar(p17.x * 0.6f + p0.x * 0.4f, p17.y * 0.4f + p0.y * 0.6f);

    res.central_palm_disc = {p5, p9, p13, p17, p_hypothenar, p0, p1, p2};

    res.is_valid = true;
    return res;
}

bool HandAnatomyDecomposer::isInsidePalmDisc(const Point2D& pt, const std::vector<Point2D>& palm_disc) {
    if (palm_disc.size() < 3) {
        return false;
    }
    bool inside = false;
    const size_t n = palm_disc.size();
    for (size_t i = 0, j = n - 1; i < n; j = i++) {
        const float xi = palm_disc[i].x;
        const float yi = palm_disc[i].y;
        const float xj = palm_disc[j].x;
        const float yj = palm_disc[j].y;

        const bool intersect =
            ((yi > pt.y) != (yj > pt.y)) && (pt.x < (xj - xi) * (pt.y - yi) / (yj - yi + 1e-7f) + xi);
        if (intersect) {
            inside = !inside;
        }
    }
    return inside;
}

} // namespace CurvEngine::domain
