#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include <opencv2/imgproc.hpp>
#include <string>
#include <vector>

namespace CurvEngine::quality {

LaplacianQualityAnalyzer::LaplacianQualityAnalyzer(QualityThresholds thresholds) : thresholds_(thresholds) {}

QualityReport LaplacianQualityAnalyzer::analyze(const Frame& frame) {
    if (frame.empty()) {
        return QualityReport{0.0, 0.0, 0.0, false, "Rejected: Input frame is empty."};
    }

    // Convert input image to single-channel 8-bit grayscale if needed
    cv::Mat gray;
    if (frame.channels() == 3) {
        cv::cvtColor(frame.image, gray, cv::COLOR_BGR2GRAY);
    } else if (frame.channels() == 4) {
        cv::cvtColor(frame.image, gray, cv::COLOR_BGRA2GRAY);
    } else if (frame.channels() == 1) {
        gray = frame.image;
    } else {
        return QualityReport{0.0, 0.0, 0.0, false, "Rejected: Unsupported channel count."};
    }

    // 1. Calculate Blur Score via Variance of Laplacian: Var(∇²I)
    cv::Mat laplacian;
    cv::Laplacian(gray, laplacian, CV_64F, 1, 1, 0, cv::BORDER_DEFAULT);

    cv::Scalar lap_mean;
    cv::Scalar lap_std;
    cv::meanStdDev(laplacian, lap_mean, lap_std);
    const double blur_variance = lap_std.val[0] * lap_std.val[0];

    // 2. Calculate Brightness and Contrast (mean & standard deviation)
    cv::Scalar gray_mean;
    cv::Scalar gray_std;
    cv::meanStdDev(gray, gray_mean, gray_std);
    const double brightness = gray_mean.val[0];
    const double contrast = gray_std.val[0];

    // 3. Evaluate Thresholds and Compile Recommendations
    std::vector<std::string> issues;

    if (blur_variance < thresholds_.min_blur_score) {
        issues.emplace_back("Image too blurry (insufficient high-frequency detail)");
    }
    if (brightness < thresholds_.min_brightness) {
        issues.emplace_back("Image too dark / underexposed");
    } else if (brightness > thresholds_.max_brightness) {
        issues.emplace_back("Image too bright / overexposed");
    }
    if (contrast < thresholds_.min_contrast) {
        issues.emplace_back("Image contrast too low (washed out or flat)");
    }

    const bool is_usable = issues.empty();
    std::string recommendation;
    if (is_usable) {
        recommendation = "Frame quality nominal; passed all quality gates.";
    } else {
        recommendation = "Rejected: ";
        for (size_t i = 0; i < issues.size(); ++i) {
            if (i > 0) recommendation += "; ";
            recommendation += issues[i];
        }
    }

    return QualityReport{blur_variance, brightness, contrast, is_usable, recommendation};
}

} // namespace CurvEngine::quality
