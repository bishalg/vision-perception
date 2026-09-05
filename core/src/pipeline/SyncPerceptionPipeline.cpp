#include "curv/pipeline/SyncPerceptionPipeline.hpp"
#include "curv/ridge/StegerRidgeExtractor.hpp"

namespace CurvEngine::pipeline {

SyncPerceptionPipeline::SyncPerceptionPipeline(std::shared_ptr<interfaces::IQualityAnalyzer> quality_analyzer,
                                               std::shared_ptr<interfaces::ILineCurveExtractor> ridge_extractor)
    : quality_analyzer_(std::move(quality_analyzer)), ridge_extractor_(std::move(ridge_extractor)) {}

PipelineResult SyncPerceptionPipeline::process(const Frame& frame) {
    const auto start_time = std::chrono::steady_clock::now();
    PipelineResult result;

    // 1. Stage 1: Quality Gate Evaluation
    if (quality_analyzer_) {
        result.quality = quality_analyzer_->analyze(frame);
        result.is_usable = result.quality.is_usable;

        // If frame is unusable, stop pipeline early to conserve compute resources
        if (!result.is_usable) {
            const auto end_time = std::chrono::steady_clock::now();
            result.total_duration_us =
                std::chrono::duration_cast<std::chrono::microseconds>(end_time - start_time).count();
            return result;
        }
    } else {
        result.is_usable = true;
    }

    // 2. Stage 2: Curvilinear Ridge Extraction
    if (ridge_extractor_) {
        // If the concrete extractor is a StegerRidgeExtractor, extract the full graph once and derive evidence
        if (auto const* steger = dynamic_cast<ridge::StegerRidgeExtractor*>(ridge_extractor_.get())) {
            result.ridge_graph = steger->extractRidgeGraph(frame.image);
            result.evidence = steger->extractFromGraph(result.ridge_graph);
        } else {
            result.evidence = ridge_extractor_->extract(frame);
        }
    }

    const auto end_time = std::chrono::steady_clock::now();
    result.total_duration_us = std::chrono::duration_cast<std::chrono::microseconds>(end_time - start_time).count();
    return result;
}

} // namespace CurvEngine::pipeline
