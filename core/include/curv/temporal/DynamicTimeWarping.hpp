#pragma once

#include <opencv2/core.hpp>
#include <string>
#include <vector>

namespace CurvEngine::temporal {

/**
 * @brief Reference template for gesture trajectory matching.
 */
struct GestureTemplate {
    std::string name;
    std::vector<cv::Point2d> trajectory;
};

/**
 * @brief Trajectory distance matching result.
 */
struct GestureMatchResult {
    std::string matched_gesture;
    double dtw_distance{1e9};
    double normalized_score{0.0}; // [0.0, 1.0] where 1.0 is exact match
};

/**
 * @brief High-performance Dynamic Time Warping (DTW) engine for comparing 2D temporal
 * gaze/gesture trajectories with optional Sakoe-Chiba constraint window.
 */
class DynamicTimeWarping {
public:
    /**
     * @brief Computes the DTW distance between two 2D trajectories.
     * @param trajectory_a First sequence of 2D points.
     * @param trajectory_b Second sequence of 2D points.
     * @param sakoe_chiba_window Maximum allowed index difference |i - j|. If <= 0, full matrix is computed.
     * @return Cumulative alignment cost.
     */
    [[nodiscard]] static double computeDistance(const std::vector<cv::Point2d>& trajectory_a,
                                                const std::vector<cv::Point2d>& trajectory_b,
                                                int sakoe_chiba_window = -1);

    /**
     * @brief Normalizes a 2D trajectory: resamples to N equidistant points, translates to origin,
     * and scales to unit bounding box.
     */
    [[nodiscard]] static std::vector<cv::Point2d> normalizeTrajectory(const std::vector<cv::Point2d>& points,
                                                                      size_t target_sample_count = 32);

    /**
     * @brief Classifies a query trajectory against a dictionary of reference gesture templates.
     */
    [[nodiscard]] static GestureMatchResult classifyGesture(const std::vector<cv::Point2d>& query_trajectory,
                                                            const std::vector<GestureTemplate>& templates,
                                                            double max_acceptance_distance = 15.0);
};

} // namespace CurvEngine::temporal
