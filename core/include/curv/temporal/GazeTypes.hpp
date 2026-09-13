#pragma once

#include <string>
#include <vector>

namespace CurvEngine::temporal {

/**
 * @brief Raw instantaneous gaze point sample from an eye tracker or mobile framework (e.g. ARKit / TrueDepth).
 */
struct GazeSample {
    double x{0.0};            ///< Horizontal coordinate (normalized [0, 1] or screen pixels)
    double y{0.0};            ///< Vertical coordinate (normalized [0, 1] or screen pixels)
    double timestamp_ms{0.0}; ///< Millisecond timestamp of sample
    double confidence{1.0};   ///< Tracking confidence [0.0, 1.0]
};

/**
 * @brief A period of relatively stationary visual attention on a specific spatial locus.
 */
struct Fixation {
    double x{0.0};                 ///< Spatial centroid X coordinate
    double y{0.0};                 ///< Spatial centroid Y coordinate
    double start_time_ms{0.0};     ///< Onset timestamp in milliseconds
    double duration_ms{0.0};       ///< Total duration of visual dwell in milliseconds
    double dispersion_radius{0.0}; ///< Maximum or RMS dispersion radius from centroid
    int sample_count{0};           ///< Number of constituent gaze samples
};

/**
 * @brief Rapid, ballistic eye movement connecting two consecutive fixations.
 */
struct Saccade {
    double start_x{0.0};       ///< Departure X coordinate
    double start_y{0.0};       ///< Departure Y coordinate
    double end_x{0.0};         ///< Destination X coordinate
    double end_y{0.0};         ///< Destination Y coordinate
    double start_time_ms{0.0}; ///< Onset timestamp in milliseconds
    double duration_ms{0.0};   ///< Flight duration in milliseconds
    double amplitude{0.0};     ///< Spatial displacement (Euclidean distance or visual degrees)
    double peak_velocity{0.0}; ///< Peak velocity during movement (px/s or deg/s)
};

/**
 * @brief Spatiotemporal sequence of alternating fixations and saccadic jumps.
 */
struct Scanpath {
    std::vector<Fixation> fixations;
    std::vector<Saccade> saccades;
    double total_duration_ms{0.0};
};

/**
 * @brief Bounded semantic region of interest on screen or visual field.
 */
struct AreaOfInterest {
    std::string id;
    double x_min{0.0};
    double y_min{0.0};
    double x_max{0.0};
    double y_max{0.0};

    [[nodiscard]] constexpr bool contains(double x, double y) const noexcept {
        return x >= x_min && x <= x_max && y >= y_min && y <= y_max;
    }
};

} // namespace CurvEngine::temporal
