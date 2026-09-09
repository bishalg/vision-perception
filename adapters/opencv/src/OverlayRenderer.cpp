#include "curv/adapters/OverlayRenderer.hpp"
#include <cmath>
#include <fstream>
#include <iomanip>
#include <opencv2/imgproc.hpp>
#include <sstream>

namespace CurvEngine::adapters {

cv::Mat OverlayRenderer::render(const cv::Mat& background, const RidgeGraph& graph, const QualityReport& quality,
                                const OverlayConfig& config) {

    cv::Mat canvas;
    if (background.empty()) {
        canvas = cv::Mat::zeros(600, 800, CV_8UC3);
    } else if (background.channels() == 1) {
        cv::cvtColor(background, canvas, cv::COLOR_GRAY2BGR);
    } else if (background.channels() == 4) {
        cv::cvtColor(background, canvas, cv::COLOR_BGRA2BGR);
    } else {
        canvas = background.clone();
    }

    const float subpixel_factor =
        static_cast<float>(1u << static_cast<unsigned>(config.subpixel_shift)); // 16.0f for shift=4

    // 1. Draw Ribbons (if requested)
    if (config.draw_ribbons && !graph.segments.empty()) {
        cv::Mat ribbon_layer = canvas.clone();
        const float half_w = config.ribbon_width * 0.5f;

        for (const auto& seg : graph.segments) {
            if (seg.points.size() < 2) continue;

            std::vector<cv::Point> polygon_pts;
            polygon_pts.reserve(seg.points.size() * 2);

            // Left edge
            for (size_t i = 0; i < seg.points.size(); ++i) {
                float tx = 0.0f;
                float ty = 1.0f;
                if (i + 1 < seg.points.size()) {
                    tx = seg.points[i + 1].x - seg.points[i].x;
                    ty = seg.points[i + 1].y - seg.points[i].y;
                } else if (i > 0) {
                    tx = seg.points[i].x - seg.points[i - 1].x;
                    ty = seg.points[i].y - seg.points[i - 1].y;
                }
                const float len = std::max(1e-6f, std::sqrt(tx * tx + ty * ty));
                const float nx = -ty / len;
                const float ny = tx / len;

                const float rx = (seg.points[i].x + nx * half_w) * subpixel_factor;
                const float ry = (seg.points[i].y + ny * half_w) * subpixel_factor;
                polygon_pts.emplace_back(static_cast<int>(std::round(rx)), static_cast<int>(std::round(ry)));
            }

            // Right edge (reversed)
            for (int i = static_cast<int>(seg.points.size()) - 1; i >= 0; --i) {
                float tx = 0.0f;
                float ty = 1.0f;
                const size_t ui = static_cast<size_t>(i);
                if (ui + 1 < seg.points.size()) {
                    tx = seg.points[ui + 1].x - seg.points[ui].x;
                    ty = seg.points[ui + 1].y - seg.points[ui].y;
                } else if (ui > 0) {
                    tx = seg.points[ui].x - seg.points[ui - 1].x;
                    ty = seg.points[ui].y - seg.points[ui - 1].y;
                }
                const float len = std::max(1e-6f, std::sqrt(tx * tx + ty * ty));
                const float nx = -ty / len;
                const float ny = tx / len;

                const float rx = (seg.points[ui].x - nx * half_w) * subpixel_factor;
                const float ry = (seg.points[ui].y - ny * half_w) * subpixel_factor;
                polygon_pts.emplace_back(static_cast<int>(std::round(rx)), static_cast<int>(std::round(ry)));
            }

            const cv::Point* pts_ptr = polygon_pts.data();
            int const n_pts = static_cast<int>(polygon_pts.size());
            cv::fillPoly(ribbon_layer, &pts_ptr, &n_pts, 1, config.ribbon_color, cv::LINE_AA, config.subpixel_shift);
        }

        // Alpha blend ribbon layer
        cv::addWeighted(ribbon_layer, 0.4, canvas, 0.6, 0.0, canvas);
    }

    // 2. Draw Hand Boundary Contours
    if (config.draw_boundary && !config.boundary_contours.empty()) {
        cv::drawContours(canvas, config.boundary_contours, -1, config.boundary_color, config.boundary_thickness,
                         cv::LINE_AA);
    }

    // 2b. Draw 2nd-Level Anatomy Lines (Digit separation rays & basal MCP crease arc)
    if (config.draw_anatomy && !config.anatomy_contours.empty()) {
        for (const auto& contour : config.anatomy_contours) {
            if (contour.size() < 2) continue;
            const cv::Point* pts_ptr = contour.data();
            int const n_pts = static_cast<int>(contour.size());
            cv::polylines(canvas, &pts_ptr, &n_pts, 1, false, config.anatomy_color, config.anatomy_thickness,
                          cv::LINE_AA);
        }
    }

    // 2c. Draw Continuous Major Crease Spline Curves (Heart, Head, Life, Fate lines)
    if (config.draw_continuous_creases && !config.continuous_creases.empty()) {
        for (const auto& item : config.continuous_creases) {
            if (item.points.size() < 2) continue;

            const cv::Point* pts_ptr = item.points.data();
            int const n_pts = static_cast<int>(item.points.size());

            // Outer soft glow
            const cv::Scalar glow_color = item.color * 0.4;
            cv::polylines(canvas, &pts_ptr, &n_pts, 1, false, glow_color, item.thickness + 4, cv::LINE_AA);

            // Core solid continuous line
            cv::polylines(canvas, &pts_ptr, &n_pts, 1, false, item.color, item.thickness, cv::LINE_AA);

            // Label badge
            if (!item.label.empty()) {
                const auto& mid_pt = item.points[item.points.size() / 2];
                const cv::Point text_origin(mid_pt.x + 12, mid_pt.y - 12);

                int baseline = 0;
                double const font_scale = std::clamp(canvas.cols / 1400.0, 0.45, 1.4);
                int const font_thickness = std::max(1, static_cast<int>(font_scale * 2.0));
                cv::Size const text_size =
                    cv::getTextSize(item.label, cv::FONT_HERSHEY_SIMPLEX, font_scale, font_thickness, &baseline);

                cv::Rect bg_rect(text_origin.x - 4, text_origin.y - text_size.height - 4, text_size.width + 8,
                                 text_size.height + 8);
                if (bg_rect.x >= 0 && bg_rect.y >= 0 && bg_rect.x + bg_rect.width < canvas.cols &&
                    bg_rect.y + bg_rect.height < canvas.rows) {
                    cv::Mat badge_roi = canvas(bg_rect);
                    cv::Mat const badge_bg(bg_rect.size(), CV_8UC3, cv::Scalar(15, 15, 15));
                    cv::addWeighted(badge_bg, 0.75, badge_roi, 0.25, 0.0, badge_roi);
                    cv::rectangle(canvas, bg_rect, item.color, 1, cv::LINE_AA);
                }
                cv::putText(canvas, item.label, text_origin, cv::FONT_HERSHEY_SIMPLEX, font_scale,
                            cv::Scalar(255, 255, 255), font_thickness, cv::LINE_AA);
            }
        }
    }

    // 3. Draw Sub-pixel Centerline Polylines (16x shift for AA)
    if (config.draw_subpixel_polylines) {
        for (const auto& seg : graph.segments) {
            if (seg.points.size() < 2) continue;

            cv::Scalar color = config.line_color;
            int thickness = config.line_thickness;
            if (auto const it = config.segment_colors.find(seg.id); it != config.segment_colors.end()) {
                color = it->second;
                thickness = std::max(thickness + 2, static_cast<int>(static_cast<float>(thickness) * 1.5f));
            }

            std::vector<cv::Point> pts;
            pts.reserve(seg.points.size());
            for (const auto& pt : seg.points) {
                pts.emplace_back(static_cast<int>(std::round(pt.x * subpixel_factor)),
                                 static_cast<int>(std::round(pt.y * subpixel_factor)));
            }

            const cv::Point* pts_ptr = pts.data();
            int const n_pts = static_cast<int>(pts.size());
            cv::polylines(canvas, &pts_ptr, &n_pts, 1, false, color, thickness, cv::LINE_AA, config.subpixel_shift);

            if (auto const it_lbl = config.segment_labels.find(seg.id);
                it_lbl != config.segment_labels.end() && !it_lbl->second.empty()) {
                const auto& mid_pt = seg.points[seg.points.size() / 2];
                const cv::Point text_origin(static_cast<int>(mid_pt.x + 10), static_cast<int>(mid_pt.y - 10));

                int baseline = 0;
                double const font_scale = std::clamp(canvas.cols / 1400.0, 0.45, 1.4);
                int const font_thickness = std::max(1, static_cast<int>(font_scale * 2.0));
                cv::Size const text_size =
                    cv::getTextSize(it_lbl->second, cv::FONT_HERSHEY_SIMPLEX, font_scale, font_thickness, &baseline);

                cv::Rect bg_rect(text_origin.x - 4, text_origin.y - text_size.height - 4, text_size.width + 8,
                                 text_size.height + 8);
                if (bg_rect.x >= 0 && bg_rect.y >= 0 && bg_rect.x + bg_rect.width < canvas.cols &&
                    bg_rect.y + bg_rect.height < canvas.rows) {
                    cv::Mat badge_roi = canvas(bg_rect);
                    cv::Mat const badge_bg(bg_rect.size(), CV_8UC3, cv::Scalar(15, 15, 15));
                    cv::addWeighted(badge_bg, 0.75, badge_roi, 0.25, 0.0, badge_roi);
                    cv::rectangle(canvas, bg_rect, color, 1, cv::LINE_AA);
                }
                cv::putText(canvas, it_lbl->second, text_origin, cv::FONT_HERSHEY_SIMPLEX, font_scale,
                            cv::Scalar(255, 255, 255), font_thickness, cv::LINE_AA);
            }
        }
    }

    // 3. Draw Normal Vectors (Whiskers)
    if (config.draw_normals) {
        for (const auto& seg : graph.segments) {
            for (size_t i = 0; i < seg.points.size(); i += static_cast<size_t>(config.normal_stride)) {
                float tx = 0.0f;
                float ty = 1.0f;
                if (i + 1 < seg.points.size()) {
                    tx = seg.points[i + 1].x - seg.points[i].x;
                    ty = seg.points[i + 1].y - seg.points[i].y;
                } else if (i > 0) {
                    tx = seg.points[i].x - seg.points[i - 1].x;
                    ty = seg.points[i].y - seg.points[i - 1].y;
                }
                const float len = std::max(1e-6f, std::sqrt(tx * tx + ty * ty));
                const float nx = -ty / len;
                const float ny = tx / len;

                const float p1x = (seg.points[i].x - nx * (config.normal_length * 0.5f)) * subpixel_factor;
                const float p1y = (seg.points[i].y - ny * (config.normal_length * 0.5f)) * subpixel_factor;
                const float p2x = (seg.points[i].x + nx * (config.normal_length * 0.5f)) * subpixel_factor;
                const float p2y = (seg.points[i].y + ny * (config.normal_length * 0.5f)) * subpixel_factor;

                cv::line(canvas, cv::Point(static_cast<int>(std::round(p1x)), static_cast<int>(std::round(p1y))),
                         cv::Point(static_cast<int>(std::round(p2x)), static_cast<int>(std::round(p2y))),
                         config.normal_color, 1, cv::LINE_AA, config.subpixel_shift);
            }
        }
    }

    // 4. Draw Diagnostics HUD Box
    if (config.draw_hud && canvas.cols >= 100 && canvas.rows >= 120) {
        const int hud_w = std::min(420, canvas.cols - 20);
        const int hud_h = std::min(105, canvas.rows - 20);
        const cv::Rect hud_rect(10, 10, hud_w, hud_h);

        // Alpha blended backdrop
        cv::Mat roi = canvas(hud_rect);
        cv::Mat const hud_bg(hud_rect.size(), CV_8UC3, cv::Scalar(20, 20, 20));
        cv::addWeighted(hud_bg, 0.75, roi, 0.25, 0.0, roi);
        cv::rectangle(canvas, hud_rect, cv::Scalar(80, 80, 80), 1, cv::LINE_AA);

        // Texts
        const cv::Scalar status_color = quality.is_usable ? cv::Scalar(0, 255, 0) : cv::Scalar(0, 0, 255);
        const std::string status_text = quality.is_usable ? "FRAME GATE: PASS" : "FRAME GATE: REJECT";

        std::ostringstream line1;
        std::ostringstream line2;
        std::ostringstream line3;
        line1 << "Quality: Blur=" << std::fixed << std::setprecision(1) << quality.blur_score
              << " | Bright=" << quality.brightness_score;
        line2 << "Contrast=" << std::fixed << std::setprecision(1) << quality.contrast_score;
        size_t total_points = 0;
        for (const auto& s : graph.segments) {
            total_points += s.points.size();
        }
        line3 << "Ridges: " << graph.segments.size() << " segments (" << total_points << " pts)";

        cv::putText(canvas, status_text, cv::Point(20, 32), cv::FONT_HERSHEY_SIMPLEX, 0.55, status_color, 1,
                    cv::LINE_AA);
        cv::putText(canvas, line1.str(), cv::Point(20, 54), cv::FONT_HERSHEY_SIMPLEX, 0.42, cv::Scalar(230, 230, 230),
                    1, cv::LINE_AA);
        cv::putText(canvas, line2.str(), cv::Point(20, 72), cv::FONT_HERSHEY_SIMPLEX, 0.42, cv::Scalar(210, 210, 210),
                    1, cv::LINE_AA);
        cv::putText(canvas, line3.str(), cv::Point(20, 94), cv::FONT_HERSHEY_SIMPLEX, 0.42, cv::Scalar(0, 255, 200), 1,
                    cv::LINE_AA);
    }

    return canvas;
}

bool OverlayRenderer::exportToSvg(const RidgeGraph& graph, int width, int height, const std::string& filepath) {

    std::ofstream svg(filepath);
    if (!svg.is_open()) {
        return false;
    }

    svg << "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n"
        << R"(<svg xmlns="http://www.w3.org/2000/svg" version="1.1" )"
        << "width=\"" << width << "\" height=\"" << height << "\" "
        << "viewBox=\"0 0 " << width << " " << height << "\">\n"
        << "  <rect width=\"100%\" height=\"100%\" fill=\"#1A1A1A\"/>\n"
        << "  <g id=\"curv_ridges\" stroke=\"#00FF66\" stroke-width=\"1.5\" fill=\"none\" stroke-linecap=\"round\" "
           "stroke-linejoin=\"round\">\n";

    for (const auto& seg : graph.segments) {
        if (seg.points.empty()) continue;

        svg << "    <polyline id=\"seg_" << seg.id << "\" points=\"";
        for (size_t i = 0; i < seg.points.size(); ++i) {
            if (i > 0) svg << " ";
            svg << std::fixed << std::setprecision(3) << seg.points[i].x << "," << seg.points[i].y;
        }
        svg << "\" />\n";
    }

    svg << "  </g>\n</svg>\n";
    return true;
}

} // namespace CurvEngine::adapters
