#include "curv/roi/CanonicalWarper.hpp"
#include <opencv2/imgproc.hpp>

namespace CurvEngine::roi {

CanonicalWarper::CanonicalWarper(int target_dimension) : target_dim_(target_dimension) {}

Frame CanonicalWarper::warpQuad(const Frame& source_frame, const std::array<Point2D, 4>& corners,
                                int output_size) const {
    const int dim = (output_size > 0) ? output_size : target_dim_;
    cv::Mat const H = geometry::computeQuadPerspectiveTransform(corners, dim);

    cv::Mat warped;
    cv::warpPerspective(source_frame.image, warped, H, cv::Size(dim, dim), cv::INTER_LINEAR, cv::BORDER_REPLICATE);

    Frame result(warped, source_frame.source_id + "_canonical_quad", source_frame.timestamp_ms);
    return result;
}

Frame CanonicalWarper::warpPalm(const Frame& source_frame, const geometry::HandLandmarks& landmarks,
                                int output_size) const {
    const int dim = (output_size > 0) ? output_size : target_dim_;
    const auto transform =
        geometry::computeCanonicalPalmTransform(landmarks, source_frame.width(), source_frame.height(), dim);

    cv::Mat const warped = geometry::warpToCanonical(source_frame.image, transform.M, dim);
    Frame result(warped, source_frame.source_id + "_canonical_palm", source_frame.timestamp_ms);
    return result;
}

} // namespace CurvEngine::roi
