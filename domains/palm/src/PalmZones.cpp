#include "curv/domain/PalmZones.hpp"

namespace CurvEngine::domains {

std::vector<geometry::Zone> PalmZones::getCanonicalPalmZones(int target_dim, bool is_left_hand) {
    const float s = static_cast<float>(target_dim) / 512.0f;
    const float t_dim = static_cast<float>(target_dim);

    auto const make_poly = [s, t_dim, is_left_hand](std::initializer_list<std::pair<float, float>> coords) {
        std::vector<Point2D> poly;
        poly.reserve(coords.size());
        for (const auto& [x, y] : coords) {
            float px = x * s;
            if (is_left_hand) {
                px = t_dim - px;
            }
            poly.emplace_back(px, y * s);
        }
        return poly;
    };

    return {{"jupiter_zone", make_poly({{100.0f, 100.0f}, {200.0f, 100.0f}, {200.0f, 220.0f}, {100.0f, 220.0f}})},
            {"saturn_zone", make_poly({{200.0f, 80.0f}, {300.0f, 80.0f}, {300.0f, 200.0f}, {200.0f, 200.0f}})},
            {"apollo_zone", make_poly({{300.0f, 90.0f}, {400.0f, 90.0f}, {400.0f, 210.0f}, {300.0f, 210.0f}})},
            {"mercury_zone", make_poly({{400.0f, 110.0f}, {490.0f, 110.0f}, {490.0f, 230.0f}, {400.0f, 230.0f}})},
            {"mars_upper", make_poly({{380.0f, 230.0f}, {490.0f, 230.0f}, {490.0f, 330.0f}, {380.0f, 330.0f}})},
            {"luna_zone", make_poly({{330.0f, 330.0f}, {480.0f, 330.0f}, {480.0f, 480.0f}, {330.0f, 480.0f}})},
            // Expanded y to 500 to capture lower life arc (user 01_left life y 470-500) — was 470
            {"venus_zone", make_poly({{50.0f, 230.0f}, {200.0f, 230.0f}, {220.0f, 500.0f}, {70.0f, 500.0f}})},
            {"mars_lower", make_poly({{80.0f, 180.0f}, {190.0f, 180.0f}, {190.0f, 260.0f}, {80.0f, 260.0f}})},
            // Expanded plain_of_mars y to 380 to cover head lower tail (user head y 327)
            {"plain_of_mars", make_poly({{190.0f, 200.0f}, {380.0f, 200.0f}, {350.0f, 380.0f}, {200.0f, 380.0f}})}};
}

} // namespace CurvEngine::domains
