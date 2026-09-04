#pragma once

#include <memory>
#include <vector>
#include <chrono>
#include "curv/Frame.hpp"
#include "curv/QualityReport.hpp"
#include "curv/GeometryTypes.hpp"
#include "curv/Evidence.hpp"
#include "curv/interfaces/IQualityAnalyzer.hpp"
#include "curv/interfaces/ILineCurveExtractor.hpp"

namespace CurvEngine::pipeline {

/**
 * @brief Aggregated result from synchronous perception pipeline execution.
 */
struct PipelineResult {
    bool is_usable{false};
    QualityReport quality;
    RidgeGraph ridge_graph;
    std::vector<Evidence> evidence;
    int64_t total_duration_us{0};
};

/**
 * @brief Synchronous pipeline coordinating Frame ingest, Quality Gating, and Ridge Extraction.
 */
class SyncPerceptionPipeline {
public:
    SyncPerceptionPipeline(
        std::shared_ptr<interfaces::IQualityAnalyzer> quality_analyzer,
        std::shared_ptr<interfaces::ILineCurveExtractor> ridge_extractor);

    [[nodiscard]] PipelineResult process(const Frame& frame);

private:
    std::shared_ptr<interfaces::IQualityAnalyzer> quality_analyzer_;
    std::shared_ptr<interfaces::ILineCurveExtractor> ridge_extractor_;
};

} // namespace CurvEngine::pipeline
