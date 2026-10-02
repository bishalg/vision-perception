#include "curv/preprocess/FlatFieldCLAHEPreprocessor.hpp"

#include <opencv2/imgproc.hpp>

#include <algorithm>
#include <cmath>

namespace CurvEngine::preprocess {
namespace {

cv::Mat toGray(const cv::Mat& image) {
    if (image.empty()) {
        return image;
    }
    if (image.channels() == 1) {
        return image;
    }
    cv::Mat gray;
    if (image.channels() == 4) {
        cv::cvtColor(image, gray, cv::COLOR_BGRA2GRAY);
    } else {
        cv::cvtColor(image, gray, cv::COLOR_BGR2GRAY);
    }
    return gray;
}

int oddAtLeast(int value, int minimum) {
    const int v = std::max(value, minimum);
    return (v % 2 == 1) ? v : v + 1;
}

} // namespace

FlatFieldCLAHEPreprocessor::FlatFieldCLAHEPreprocessor(FlatFieldCLAHEConfig config) : config_(config) {}

cv::Mat FlatFieldCLAHEPreprocessor::apply(const cv::Mat& image, float palm_width_px) const {
    const cv::Mat gray = toGray(image);
    if (gray.empty() || !config_.enabled) {
        return gray;
    }

    // Background estimate: a closing with a kernel far larger than crease width follows
    // the low-frequency illumination while ignoring the dark thin lines themselves.
    float scale = palm_width_px;
    if (scale <= 0.0F) {
        scale = 0.20F * static_cast<float>(std::min(gray.cols, gray.rows));
    }
    const int kernel = oddAtLeast(static_cast<int>(config_.close_kernel_frac * scale), config_.close_kernel_min_px);
    cv::Mat background;
    cv::morphologyEx(gray, background, cv::MORPH_CLOSE,
                     cv::getStructuringElement(cv::MORPH_ELLIPSE, cv::Size(kernel, kernel)));

    // Flatten: divide out the illumination, flooring the background so dark regions
    // do not amplify sensor noise.
    cv::Mat gray_f;
    gray.convertTo(gray_f, CV_32F);
    cv::Mat background_f;
    background.convertTo(background_f, CV_32F);
    background_f = cv::max(background_f, cv::Scalar(static_cast<double>(config_.background_floor)));
    cv::Mat flat_f;
    cv::divide(gray_f, background_f, flat_f);
    flat_f *= 255.0;

    cv::Mat flat;
    flat_f.convertTo(flat, CV_8U);

    // Local contrast lift for faint secondary creases. Skippable: CLAHE lifts the major
    // creases but also multiplies skin micro-texture, which collapses micro-sign detection
    // on the canonical capture. Flattening alone is sign-safe.
    if (!config_.enable_clahe) {
        return flat;
    }
    auto clahe = cv::createCLAHE(config_.clahe_clip, cv::Size(config_.clahe_tiles, config_.clahe_tiles));
    cv::Mat out;
    clahe->apply(flat, out);
    return out;
}

Frame FlatFieldCLAHEPreprocessor::process(const Frame& frame) {
    // Frame-level entry has no landmark context; use the canonical-patch convention
    // (palm width ~55% of the patch) unless callers use apply() directly.
    const float palm_width = 0.55F * static_cast<float>(std::min(frame.image.cols, frame.image.rows));
    Frame out(apply(frame.image, palm_width), frame.source_id, frame.timestamp_ms);
    out.metadata = frame.metadata;
    return out;
}

} // namespace CurvEngine::preprocess
