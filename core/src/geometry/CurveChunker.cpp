#include "curv/geometry/CurveChunker.hpp"
#include <algorithm>
#include <cmath>

namespace CurvEngine::geometry {

namespace {

float distance2D(const Point2D& a, const Point2D& b) {
    const float dx = b.x - a.x;
    const float dy = b.y - a.y;
    return std::sqrt(dx * dx + dy * dy);
}

void computeArcLengths(const std::vector<Point2D>& pts, std::vector<float>& cumulative_lengths, float& total_len) {
    cumulative_lengths.clear();
    total_len = 0.0f;
    if (pts.empty()) return;

    cumulative_lengths.reserve(pts.size());
    cumulative_lengths.push_back(0.0f);

    for (size_t i = 1; i < pts.size(); ++i) {
        total_len += distance2D(pts[i - 1], pts[i]);
        cumulative_lengths.push_back(total_len);
    }
}

} // namespace

Point2D PartitionedCurve::sampleAt(float s) const {
    const float clamped_s = std::clamp(s, 0.0f, 1.0f);
    if (cumulative_points.empty()) return {0.0f, 0.0f};
    if (cumulative_points.size() == 1 || total_arc_length <= 1e-6f) return cumulative_points.front();

    const float target_len = clamped_s * total_arc_length;

    // Binary search for segment
    auto it = std::lower_bound(cumulative_lengths.begin(), cumulative_lengths.end(), target_len);
    if (it == cumulative_lengths.begin()) return cumulative_points.front();
    if (it == cumulative_lengths.end()) return cumulative_points.back();

    auto dist = std::distance(cumulative_lengths.begin(), it);
    size_t idx = static_cast<size_t>(dist);
    float l0 = cumulative_lengths[idx - 1];
    float l1 = cumulative_lengths[idx];
    float seg_len = l1 - l0;
    float t = (seg_len > 1e-6f) ? (target_len - l0) / seg_len : 0.0f;

    const Point2D& p0 = cumulative_points[idx - 1];
    const Point2D& p1 = cumulative_points[idx];

    return {p0.x + t * (p1.x - p0.x), p0.y + t * (p1.y - p0.y), p0.intensity + t * (p1.intensity - p0.intensity),
            p0.confidence + t * (p1.confidence - p0.confidence)};
}

Point2D PartitionedCurve::tangentAt(float s) const {
    const float eps = 0.01f;
    float s0 = std::max(0.0f, s - eps);
    float s1 = std::min(1.0f, s + eps);
    if (s1 - s0 < 1e-6f) return {1.0f, 0.0f};

    Point2D p0 = sampleAt(s0);
    Point2D p1 = sampleAt(s1);
    float dx = p1.x - p0.x;
    float dy = p1.y - p0.y;
    float len = std::sqrt(dx * dx + dy * dy);
    if (len <= 1e-6f) return {1.0f, 0.0f};
    return {dx / len, dy / len};
}

Point2D PartitionedCurve::normalAt(float s) const {
    Point2D tan = tangentAt(s);
    // 90-degree counter-clockwise normal (-y, x)
    return {-tan.y, tan.x};
}

int PartitionedCurve::chunkIndexForArc(float s) const {
    if (chunks.empty()) return -1;
    const float clamped_s = std::clamp(s, 0.0f, 1.0f);
    for (const auto& c : chunks) {
        if (clamped_s >= c.s_start && clamped_s <= c.s_end) {
            return c.index;
        }
    }
    return static_cast<int>(chunks.size() - 1);
}

PartitionedCurve CurveChunker::divideUniform(const std::vector<Point2D>& points, int num_chunks, float val_start,
                                             float val_end) {

    PartitionedCurve result;
    if (points.size() < 2 || num_chunks <= 0) {
        return result;
    }

    result.cumulative_points = points;
    computeArcLengths(points, result.cumulative_lengths, result.total_arc_length);
    if (result.total_arc_length <= 1e-6f) {
        return result;
    }

    result.chunks.reserve(static_cast<size_t>(num_chunks));
    const float chunk_ds = 1.0f / static_cast<float>(num_chunks);

    for (int i = 0; i < num_chunks; ++i) {
        CurveChunk chunk;
        chunk.index = i;
        float fi = static_cast<float>(i);
        chunk.s_start = fi * chunk_ds;
        chunk.s_end = (i == num_chunks - 1) ? 1.0f : (fi + 1.0f) * chunk_ds;
        chunk.arc_length_px = (chunk.s_end - chunk.s_start) * result.total_arc_length;

        float s_mid = 0.5f * (chunk.s_start + chunk.s_end);
        chunk.start_pt = result.sampleAt(chunk.s_start);
        chunk.mid_pt = result.sampleAt(s_mid);
        chunk.end_pt = result.sampleAt(chunk.s_end);

        chunk.tangent = result.tangentAt(s_mid);
        chunk.normal = result.normalAt(s_mid);

        chunk.scalar_value = val_start + s_mid * (val_end - val_start);
        chunk.label = std::to_string(static_cast<int>(std::round(chunk.scalar_value)));

        result.chunks.push_back(chunk);
    }

    return result;
}

PartitionedCurve CurveChunker::divideByAnchors(const std::vector<Point2D>& points,
                                               const std::vector<CalibrationAnchor>& anchors,
                                               int sub_chunks_per_interval) {

    PartitionedCurve result;
    if (points.size() < 2 || anchors.size() < 2 || sub_chunks_per_interval <= 0) {
        return result;
    }

    result.cumulative_points = points;
    computeArcLengths(points, result.cumulative_lengths, result.total_arc_length);
    if (result.total_arc_length <= 1e-6f) {
        return result;
    }

    // Sort anchors by s
    auto sorted_anchors = anchors;
    std::sort(sorted_anchors.begin(), sorted_anchors.end(),
              [](const CalibrationAnchor& a, const CalibrationAnchor& b) { return a.s < b.s; });

    // Ensure boundary anchors
    if (sorted_anchors.front().s > 0.0f) {
        sorted_anchors.insert(sorted_anchors.begin(), {0.0f, sorted_anchors.front().scalar});
    }
    if (sorted_anchors.back().s < 1.0f) {
        sorted_anchors.push_back({1.0f, sorted_anchors.back().scalar});
    }

    int global_idx = 0;
    for (size_t interval = 0; interval + 1 < sorted_anchors.size(); ++interval) {
        const auto& a0 = sorted_anchors[interval];
        const auto& a1 = sorted_anchors[interval + 1];

        const float span_s = a1.s - a0.s;
        const float span_val = a1.scalar - a0.scalar;
        if (span_s <= 1e-6f) continue;

        const float sub_ds = span_s / static_cast<float>(sub_chunks_per_interval);

        for (int j = 0; j < sub_chunks_per_interval; ++j) {
            CurveChunk chunk;
            chunk.index = global_idx++;
            float fj = static_cast<float>(j);
            chunk.s_start = a0.s + fj * sub_ds;
            chunk.s_end = (j == sub_chunks_per_interval - 1 && interval + 2 == sorted_anchors.size())
                              ? a1.s
                              : (a0.s + (fj + 1.0f) * sub_ds);
            chunk.arc_length_px = (chunk.s_end - chunk.s_start) * result.total_arc_length;

            float s_mid = 0.5f * (chunk.s_start + chunk.s_end);
            chunk.start_pt = result.sampleAt(chunk.s_start);
            chunk.mid_pt = result.sampleAt(s_mid);
            chunk.end_pt = result.sampleAt(chunk.s_end);

            chunk.tangent = result.tangentAt(s_mid);
            chunk.normal = result.normalAt(s_mid);

            float local_t = (s_mid - a0.s) / span_s;
            chunk.scalar_value = a0.scalar + local_t * span_val;
            chunk.label = std::to_string(static_cast<int>(std::round(chunk.scalar_value)));

            result.chunks.push_back(chunk);
        }
    }

    return result;
}

} // namespace CurvEngine::geometry
