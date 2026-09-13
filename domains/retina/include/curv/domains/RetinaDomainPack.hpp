#pragma once

#include "curv/interfaces/IDomainPack.hpp"
#include <opencv2/core.hpp>

namespace CurvEngine::domains {

/**
 * @brief Domain pack for retinal fundus vessel analysis, tortuosity quantification,
 * and microvascular biomarker extraction.
 */
class RetinaDomainPack final : public IDomainPack {
public:
    explicit RetinaDomainPack(float min_vessel_length_px = 15.0f);
    ~RetinaDomainPack() override = default;

    [[nodiscard]] std::string name() const noexcept override { return "org.curv.domain.retina"; }
    [[nodiscard]] std::string version() const noexcept override { return "0.1.0"; }

    [[nodiscard]] DomainOutput process(const DomainInput& input) override;

    /**
     * @brief Preprocesses a color fundus image: extracts the green channel and optionally
     * applies CLAHE (Contrast-Limited Adaptive Histogram Equalization).
     */
    [[nodiscard]] static cv::Mat prepareGreenChannel(const cv::Mat& bgr, bool apply_clahe = true);

    /**
     * @brief Computes the arc-chord ratio tortuosity metric for a curvilinear ridge segment.
     * tau = arc_length / chord_length. Values >= 1.0; straight line = 1.0.
     */
    [[nodiscard]] static double computeArcChordTortuosity(const CurveSegment& seg);

private:
    float min_vessel_length_px_{15.0f};
};

} // namespace CurvEngine::domains
