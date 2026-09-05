#pragma once

#include "curv/GeometryTypes.hpp"
#include "curv/QualityReport.hpp"
#include <opencv2/core.hpp>
#include <string>
#include <vector>

namespace CurvEngine::adapters {

/**
 * @brief Visual rendering configuration for the sub-pixel overlay generator.
 */
struct OverlayConfig {
    bool draw_subpixel_polylines{true};
    bool draw_normals{false};
    bool draw_ribbons{false};
    bool draw_hud{true};
    cv::Scalar line_color{0, 255, 0};     ///< Green BGR
    cv::Scalar normal_color{0, 255, 255}; ///< Yellow BGR
    cv::Scalar ribbon_color{50, 205, 50}; ///< Lime Green BGR
    int line_thickness{2};
    float ribbon_width{3.0f};
    int normal_stride{6}; ///< Draw normal whisker every N points
    float normal_length{6.0f};
    int subpixel_shift{4}; ///< 2^4 = 16x fixed-point sub-pixel scale
};

/**
 * @brief Anti-aliased sub-pixel overlay renderer and vector SVG exporter.
 */
class OverlayRenderer {
public:
    /**
     * @brief Renders anti-aliased sub-pixel vectors and diagnostics HUD over an image.
     */
    [[nodiscard]] static cv::Mat render(const cv::Mat& background, const RidgeGraph& graph,
                                        const QualityReport& quality, const OverlayConfig& config = OverlayConfig{});

    /**
     * @brief Exports ridge graph polylines to a standard scalable vector graphics (SVG) file.
     */
    static bool exportToSvg(const RidgeGraph& graph, int width, int height, const std::string& filepath);
};

} // namespace CurvEngine::adapters
