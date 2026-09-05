#include "curv/geometry/RoiWarper.hpp"
#include <algorithm>
#include <cmath>
#include <opencv2/imgproc.hpp>
#if __has_include(<opencv2/geometry/2d.hpp>)
#include <opencv2/geometry/2d.hpp>
#endif

namespace CurvEngine::geometry {

RoiTransform computeCanonicalPalmTransform(const HandLandmarks& lms, int img_width, int img_height, int target_dim) {
    RoiTransform result;
    result.target_dim = target_dim;

    const float w_f = static_cast<float>(std::max(1, img_width));
    const float h_f = static_cast<float>(std::max(1, img_height));

    // Helper lambda to denormalize a landmark point
    auto const get_pt = [&](int idx) -> cv::Point2f {
        const auto& pt = lms.points[static_cast<size_t>(idx)];
        if (lms.is_normalized) {
            return {pt.x * w_f, pt.y * h_f};
        }
        return {pt.x, pt.y};
    };

    const cv::Point2f p0 = get_pt(0);   // Wrist
    const cv::Point2f p5 = get_pt(5);   // Index MCP
    const cv::Point2f p9 = get_pt(9);   // Middle MCP
    const cv::Point2f p13 = get_pt(13); // Ring MCP
    const cv::Point2f p17 = get_pt(17); // Pinky MCP

    // 1. Palm Centroid: Mean of boundary MCPs and wrist
    const cv::Point2f centroid = (p0 + p5 + p9 + p13 + p17) * 0.2f;
    result.center = centroid;

    // 2. Orientation Vector: Wrist (P0) -> Middle MCP (P9)
    const float dx = p9.x - p0.x;
    const float dy = p9.y - p0.y;
    const float vec_len = std::sqrt(dx * dx + dy * dy);

    float angle_rad = 0.0f;
    if (vec_len > 1e-4f) {
        // Angle theta in screen coords (Y downwards)
        const float theta = std::atan2(dy, dx);
        // We want vector to point vertically upwards in image space (towards -Y, angle -pi/2)
        // Rotation phi = theta + pi / 2
        angle_rad = theta + 1.5707963267948966f;
    }
    result.angle_deg = angle_rad * 57.29577951308232f; // radians to degrees

    // 3. Scale Estimation: Palm disc width ||P5 - P17||
    const float palm_w_dx = p17.x - p5.x;
    const float palm_w_dy = p17.y - p5.y;
    const float palm_width = std::sqrt(palm_w_dx * palm_w_dx + palm_w_dy * palm_w_dy);

    // Target width inside canonical patch with fixed comfortable margin
    const float target_palm_width = static_cast<float>(target_dim) * 0.55f;
    float scale = (palm_width > 1e-4f) ? (target_palm_width / palm_width) : 1.0f;
    scale = std::clamp(scale, 0.01f, 100.0f);
    result.scale = scale;

    // 4. Construct 2x3 Affine Transformation Matrix
    // Maps centroid to canonical center (target_dim / 2, target_dim / 2)
    const float target_c = static_cast<float>(target_dim) * 0.5f;
    const float alpha = scale * std::cos(angle_rad);
    const float beta = scale * std::sin(angle_rad);

    result.M = cv::Mat::zeros(2, 3, CV_64F);
    double* m_data = result.M.ptr<double>();
    m_data[0] = static_cast<double>(alpha);
    m_data[1] = static_cast<double>(beta);
    m_data[2] = static_cast<double>(target_c - (alpha * centroid.x + beta * centroid.y));
    m_data[3] = static_cast<double>(-beta);
    m_data[4] = static_cast<double>(alpha);
    m_data[5] = static_cast<double>(target_c - (-beta * centroid.x + alpha * centroid.y));

    // 5. Analytical inverse affine matrix (canonical -> original)
    // For M = [a, b, c; d, e, f], D = a*e - b*d
    const double a = m_data[0];
    const double b = m_data[1];
    const double c = m_data[2];
    const double d = m_data[3];
    const double e = m_data[4];
    const double f = m_data[5];
    const double D = a * e - b * d;

    result.M_inv = cv::Mat::zeros(2, 3, CV_64F);
    if (std::abs(D) > 1e-12) {
        double* mi = result.M_inv.ptr<double>();
        mi[0] = e / D;
        mi[1] = -b / D;
        mi[2] = (b * f - c * e) / D;
        mi[3] = -d / D;
        mi[4] = a / D;
        mi[5] = (c * d - a * f) / D;
    }

    return result;
}

bool detectIsLeftHand(const HandLandmarks& lms) {
    const auto& p0 = lms.points[0];   // Wrist
    const auto& p5 = lms.points[5];   // Index MCP
    const auto& p9 = lms.points[9];   // Middle MCP
    const auto& p17 = lms.points[17]; // Pinky MCP

    // Up vector: Wrist (P0) -> Middle MCP (P9)
    const float up_x = p9.x - p0.x;
    const float up_y = p9.y - p0.y;
    const float up_len = std::sqrt(up_x * up_x + up_y * up_y);
    if (up_len < 1e-4f) {
        return false;
    }

    // Right transverse vector (perpendicular to up in image coords where Y is down)
    // For up = (0, -1) [pointing up], right = (1, 0) [pointing right]
    const float right_x = -up_y / up_len;
    const float right_y = up_x / up_len;

    // Vector from Index (P5) to Pinky (P17)
    const float palm_w_x = p17.x - p5.x;
    const float palm_w_y = p17.y - p5.y;

    // Dot product: if Pinky is to the right of Index -> Right Hand (> 0)
    // If Pinky is to the left of Index -> Left Hand (< 0)
    const float dot = palm_w_x * right_x + palm_w_y * right_y;
    return dot < 0.0f;
}

cv::Mat computeQuadPerspectiveTransform(const std::array<Point2D, 4>& corners, int target_dim) {
    const float t_dim = static_cast<float>(target_dim);
    std::vector<cv::Point2f> const src_pts = {
        cv::Point2f(corners[0].x, corners[0].y), cv::Point2f(corners[1].x, corners[1].y),
        cv::Point2f(corners[2].x, corners[2].y), cv::Point2f(corners[3].x, corners[3].y)};

    std::vector<cv::Point2f> const dst_pts = {cv::Point2f(0.0f, 0.0f), cv::Point2f(t_dim, 0.0f),
                                              cv::Point2f(t_dim, t_dim), cv::Point2f(0.0f, t_dim)};

    return cv::getPerspectiveTransform(src_pts, dst_pts);
}

cv::Mat warpToCanonical(const cv::Mat& src, const cv::Mat& M, int target_dim) {
    cv::Mat dst;
    if (src.empty() || M.empty()) {
        return dst;
    }
    cv::warpAffine(src, dst, M, cv::Size(target_dim, target_dim), cv::INTER_LINEAR, cv::BORDER_REPLICATE);
    return dst;
}

void invertGraphCoordinates(RidgeGraph& graph, const cv::Mat& M_inv) {
    if (M_inv.empty() || M_inv.rows < 2 || M_inv.cols < 3) {
        return;
    }

    cv::Mat M_inv_64;
    if (M_inv.type() != CV_64F) {
        M_inv.convertTo(M_inv_64, CV_64F);
    } else {
        M_inv_64 = M_inv;
    }

    const double* m = M_inv_64.ptr<double>();
    const double m00 = m[0];
    const double m01 = m[1];
    const double m02 = m[2];
    const double m10 = m[3];
    const double m11 = m[4];
    const double m12 = m[5];

    // Scaling factor applied to lengths
    const float inv_scale = static_cast<float>(std::sqrt(m00 * m00 + m10 * m10));

    for (auto& seg : graph.segments) {
        seg.total_length *= inv_scale;

        for (auto& pt : seg.points) {
            const double px = static_cast<double>(pt.x);
            const double py = static_cast<double>(pt.y);

            pt.x = static_cast<float>(m00 * px + m01 * py + m02);
            pt.y = static_cast<float>(m10 * px + m11 * py + m12);
        }
    }
}

} // namespace CurvEngine::geometry
