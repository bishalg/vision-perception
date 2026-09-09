#pragma once

#include "curv/GeometryTypes.hpp"
#include "curv/QualityReport.hpp"
#include "curv/domain/CreaseChainLinker.hpp"
#include "curv/domain/HandAnatomyDecomposer.hpp"
#include "curv/geometry/RoiWarper.hpp"
#include <nlohmann/json.hpp>
#include <opencv2/core.hpp>
#include <ostream>
#include <string>
#include <unordered_map>

namespace CurvEngine::tools {

struct PalmInspectOptions {
    int canonical_dim{512};
    bool is_left_hand_hint{false};
    bool debug_all_ridges{false};
    bool draw_landmarks{true};
    float palm_disc_min_inside{0.40f};
    float mask_min_inside{0.65f};
    domain::CreaseLinkOptions link_options{};
    std::string auto_landmarks_model{}; ///< empty = default detector / heuristic
};

struct PalmInspectResult {
    bool ok{false};
    std::string error;
    cv::Mat source_bgr;
    cv::Mat canonical_bgr;
    cv::Mat canonical_overlay;
    cv::Mat full_overlay;
    geometry::HandLandmarks landmarks{};
    geometry::HandLandmarks landmarks_canonical{};
    geometry::RoiTransform transform{};
    bool is_left_hand{false};
    RidgeGraph ridge_graph{};
    QualityReport quality{};
    std::unordered_map<std::string, domain::ChainedCreaseLine> major_lines{};
    domain::HandAnatomyResult anatomy_raw{};
    domain::HandAnatomyResult anatomy_canonical{};
    nlohmann::json evidence{};
    size_t segments_after_mask{0};
    size_t segments_after_palm_disc{0};
};

/**
 * @brief Load landmarks JSON array of {x,y} or [x,y] (MediaPipe 21-point order).
 */
bool loadLandmarksJson(const std::string& path, geometry::HandLandmarks& out, std::string* err = nullptr);

/**
 * @brief Save landmarks JSON for mobile/desktop round-trips.
 */
bool saveLandmarksJson(const std::string& path, const geometry::HandLandmarks& lms, std::string* err = nullptr);

/**
 * @brief Warp source-image landmarks into canonical patch pixel space.
 */
geometry::HandLandmarks warpLandmarksToCanonical(const geometry::HandLandmarks& src, const geometry::RoiTransform& xf,
                                                   int img_w, int img_h);

/**
 * @brief Core palm inspect pipeline used by CLI and the desktop debug viewer.
 */
PalmInspectResult runPalmInspect(const cv::Mat& bgr, const geometry::HandLandmarks* landmarks_or_null,
                                 const PalmInspectOptions& options, std::ostream* log = nullptr);

/**
 * @brief CLI wrapper (hand_inspector_cli).
 */
int runHandInspector(int argc, char* argv[], std::ostream& out, std::ostream& err);

/**
 * @brief Interactive OpenCV HighGUI debug viewer for crease overlays.
 */
int runPalmOverlayDebug(int argc, char* argv[], std::ostream& out, std::ostream& err);

} // namespace CurvEngine::tools
