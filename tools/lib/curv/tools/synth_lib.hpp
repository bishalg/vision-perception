#pragma once

#include <cstdint>
#include <filesystem>
#include <opencv2/core.hpp>
#include <ostream>
#include <string>
#include <vector>

namespace CurvEngine::tools {

/**
 * @brief Parameters describing a synthetic curvilinear ground-truth scene.
 */
struct SyntheticParams {
    int width = 1024;
    int height = 1024;
    double thickness_px = 3.0;
    double curve_brightness = 220.0;
    double background_brightness = 20.0;
    double noise_sigma = 0.0;
    uint64_t seed = 12345;
    double sample_step_px = 0.25; ///< Ground-truth arc-length sampling step.
};

/**
 * @brief Dense analytical sub-pixel samples for "line" | "parabola" | "spiral"
 *        (empty vector for an unknown type), spaced p.sample_step_px apart.
 */
[[nodiscard]] std::vector<cv::Point2d> sampleGroundTruth(const std::string& type, const SyntheticParams& p);

/**
 * @brief Draws the dense polyline with fixed-point sub-pixel anti-aliased segments.
 */
void drawCurve(cv::Mat& img, const std::vector<cv::Point2d>& pts, const SyntheticParams& p);

/**
 * @brief Adds deterministic Gaussian noise (no-op when sigma <= 0).
 */
void addGaussianNoise(cv::Mat& img, double sigma, uint64_t seed);

/**
 * @brief Writes <type>_image.png and <type>_truth.json into out_dir.
 * @param err optional error sink (defaults to std::cerr when null).
 * @return false when either output cannot be written.
 */
bool writeOutputs(const std::string& type, const std::filesystem::path& out_dir, const cv::Mat& img,
                  const std::vector<cv::Point2d>& pts, const SyntheticParams& p, std::ostream* err = nullptr);

/**
 * @brief CLI runner for the synthetic ground-truth generator.
 * @return process-style exit code (0 = success).
 */
int runSynthGenerator(int argc, char* argv[], std::ostream& out, std::ostream& err);

} // namespace CurvEngine::tools
