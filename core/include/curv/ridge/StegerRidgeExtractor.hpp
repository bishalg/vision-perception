#pragma once

#include "curv/Evidence.hpp"
#include "curv/GeometryTypes.hpp"
#include "curv/interfaces/ILineCurveExtractor.hpp"
#include <memory>
#include <opencv2/core.hpp>
#include <vector>

namespace CurvEngine::ridge {

/**
 * @brief Algorithmic parameters for Steger sub-pixel curvilinear ridge extraction.
 */
struct StegerConfig {
    float sigma{1.5f};              ///< Scale-space Gaussian standard deviation
    float low_threshold{0.5f};      ///< Hysteresis lower salience bound (|lambda1|)
    float high_threshold{1.5f};     ///< Hysteresis seed salience bound (|lambda1|)
    bool extract_dark_lines{false}; ///< True for dark valleys/creases; False for bright ridges
    float min_segment_length{5.0f}; ///< Minimum polyline arc-length (pixels) to retain
};

/**
 * @brief Concrete curvilinear structure extractor based on Carsten Steger's 1998 algorithm.
 */
class StegerRidgeExtractor final : public interfaces::ILineCurveExtractor {
public:
    explicit StegerRidgeExtractor(StegerConfig config = StegerConfig{});
    ~StegerRidgeExtractor() override = default;

    /**
     * @brief Extracts a complete topological RidgeGraph from an input raster image.
     */
    [[nodiscard]] RidgeGraph extractRidgeGraph(const cv::Mat& image) const;

    /**
     * @brief Implements the abstract ILineCurveExtractor interface.
     */
    [[nodiscard]] std::vector<Evidence> extract(const Frame& frame) override;

    /**
     * @brief Converts an already extracted RidgeGraph directly into Evidence items without re-running extraction.
     */
    [[nodiscard]] std::vector<Evidence> extractFromGraph(const RidgeGraph& graph) const;

    [[nodiscard]] const StegerConfig& getConfig() const noexcept { return config_; }
    void setConfig(const StegerConfig& config) noexcept { config_ = config; }

private:
    StegerConfig config_;
};

} // namespace CurvEngine::ridge
