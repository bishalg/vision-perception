#pragma once

#include "curv/temporal/GazeTypes.hpp"
#include <string>
#include <vector>

namespace CurvEngine::temporal {

/**
 * @brief Configuration parameters for I-VT (Velocity-Threshold Identification) classifier.
 */
struct IvtConfig {
    double velocity_threshold{150.0};      ///< Threshold velocity (units/sec). Below is fixation, above is saccade.
    double min_fixation_duration_ms{80.0}; ///< Minimum duration to qualify a cluster as a fixation.
};

/**
 * @brief Configuration parameters for I-DT (Dispersion-Threshold Identification) classifier.
 */
struct IdtConfig {
    double dispersion_threshold{30.0}; ///< Maximum bounding box dispersion (max(x)-min(x) + max(y)-min(y)).
    double window_duration_ms{100.0};  ///< Sliding window duration threshold (ms).
};

/**
 * @brief Classifies raw temporal gaze coordinate streams into discrete visual events
 * (Fixations, Saccades) and symbolic scanpaths using canonical I-VT / I-DT algorithms.
 */
class OculomotorClassifier {
public:
    /**
     * @brief Parses gaze samples into a Scanpath using Velocity-Threshold Identification (I-VT).
     */
    [[nodiscard]] static Scanpath classifyIvt(const std::vector<GazeSample>& samples, const IvtConfig& config = {});

    /**
     * @brief Parses gaze samples into a Scanpath using Dispersion-Threshold Identification (I-DT).
     */
    [[nodiscard]] static Scanpath classifyIdt(const std::vector<GazeSample>& samples, const IdtConfig& config = {});

    /**
     * @brief Encodes fixations in a scanpath into a symbolic character string representing
     * the sequential traversal through Areas of Interest (AOIs).
     * e.g., "AABBC" -> "ABC" (deduplicated or full).
     */
    [[nodiscard]] static std::string encodeAoiSequence(const Scanpath& scanpath,
                                                       const std::vector<AreaOfInterest>& aois,
                                                       bool deduplicate_consecutive = true);

    /**
     * @brief Computes the ratio of total fixation dwell time that was spent outside
     * a primary Area of Interest (useful for distraction and drowsiness detection).
     */
    [[nodiscard]] static double computeOffTargetDwellRatio(const Scanpath& scanpath,
                                                           const AreaOfInterest& primary_target);
};

} // namespace CurvEngine::temporal
