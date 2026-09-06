#include "curv/tools/demo_lib.hpp"

#include <iostream>
#include <opencv2/core.hpp>
#include <spdlog/spdlog.h>

#include "curv/Evidence.hpp"
#include "curv/Frame.hpp"
#include "curv/QualityReport.hpp"
#include "curv/Version.hpp"

namespace CurvEngine::tools {

int runDesktopDemo() {
    // 1. Initialize spdlog log level and pattern
    spdlog::set_level(spdlog::level::info);
    spdlog::set_pattern("[%Y-%m-%d %H:%M:%S.%e] [%^%l%$] %v");

    // 2. Print startup banner
    spdlog::info("==================================================");
    spdlog::info("vision-perception desktop pipeline demo");
    spdlog::info("CurvEngine Core Version: {}", CurvEngine::getVersion());

    // 3. Print OpenCV version
    spdlog::info("OpenCV Runtime Version: {}", cv::getVersionString());
    spdlog::info("==================================================");

    // 4. Create a dummy Frame (blank 800x600 black cv::Mat)
    cv::Mat const blank_canvas = cv::Mat::zeros(600, 800, CV_8UC3);
    CurvEngine::Frame frame(blank_canvas, "simulated_synthetic_sensor_0", 1725410000000ULL,
                            {{"environment", "desktop_demo"}, {"format", "BGR8"}});
    spdlog::info("Ingested frame: {}x{} ({} channels), source: '{}'", frame.width(), frame.height(), frame.channels(),
                 frame.source_id);

    // 5. Create a dummy QualityReport
    CurvEngine::QualityReport quality(0.0,   // blur_score
                                      128.5, // brightness_score
                                      45.2,  // contrast_score
                                      true,  // is_usable
                                      "Frame illumination and focus within nominal bounds");

    // 6. Create dummy Evidence object of type "quality_report"
    nlohmann::json const quality_geometry = {
        {"roi", {{"x", 0}, {"y", 0}, {"width", frame.width()}, {"height", frame.height()}}}};

    nlohmann::json const quality_metadata = {{"blur_score", quality.blur_score},
                                             {"brightness_score", quality.brightness_score},
                                             {"contrast_score", quality.contrast_score},
                                             {"is_usable", quality.is_usable},
                                             {"recommendation", quality.recommendation}};

    CurvEngine::Evidence const evidence("ev_demo_quality_001", "quality_report", 0.98, quality_geometry,
                                        quality_metadata);

    // 7. Serialize Evidence to JSON and print to console using spdlog::info
    nlohmann::json const evidence_json = evidence.to_json();
    spdlog::info("Perception Evidence Generated:\n{}", evidence_json.dump(2));

    // 8. Exit successfully
    spdlog::info("Pipeline execution finished successfully.");
    return 0;
}

} // namespace CurvEngine::tools
