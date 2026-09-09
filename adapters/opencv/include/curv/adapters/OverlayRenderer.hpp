#pragma once

#include "curv/GeometryTypes.hpp"
#include "curv/QualityReport.hpp"
#include <opencv2/core.hpp>
#include <string>
#include <unordered_map>
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
    int subpixel_shift{4};                                   ///< 2^4 = 16x fixed-point sub-pixel scale
    std::unordered_map<int, cv::Scalar> segment_colors{};    ///< Optional custom color per segment ID
    std::unordered_map<int, std::string> segment_labels{};   ///< Optional text label per segment ID
    bool draw_boundary{true};                                ///< Draw hand boundary contours if provided
    cv::Scalar boundary_color{255, 200, 50};                 ///< Cyan / Light Aqua Blue BGR
    int boundary_thickness{2};                               ///< Boundary line thickness
    std::vector<std::vector<cv::Point>> boundary_contours{}; ///< Boundary contour polylines to render

    // 2nd-Level Anatomy Lines (Digit separation rays & basal MCP crease arc)
    bool draw_anatomy{false};
    std::vector<std::vector<cv::Point>> anatomy_contours{};
    cv::Scalar anatomy_color{30, 160, 255}; ///< Warm Amber / Gold BGR
    int anatomy_thickness{2};

    // Continuous Major Crease Curves (Chained splines for Heart, Head, Life, Fate lines)
    struct ContinuousCreaseItem {
        std::vector<cv::Point> points{};
        cv::Scalar color{50, 205, 50};
        std::string label{};
        int thickness{3};
    };
    bool draw_continuous_creases{false};
    std::vector<ContinuousCreaseItem> continuous_creases{};
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
