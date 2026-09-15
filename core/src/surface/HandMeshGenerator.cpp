#include "curv/surface/HandMeshGenerator.hpp"
#include <algorithm>
#include <cmath>
#include <sstream>

namespace CurvEngine::surface {

namespace {

std::array<float, 3> elevationColor(float t) {
    const float clamped = std::max(0.0f, std::min(1.0f, t));
    if (clamped < 0.33f) {
        const float s = clamped / 0.33f;
        return {0.1f + 0.0f * s, 0.4f + 0.4f * s, 0.9f - 0.2f * s}; // Blue to Cyan
    } else if (clamped < 0.66f) {
        const float s = (clamped - 0.33f) / 0.33f;
        return {0.1f + 0.85f * s, 0.8f - 0.05f * s, 0.7f - 0.6f * s}; // Cyan to Gold
    } else {
        const float s = (clamped - 0.66f) / 0.34f;
        return {0.95f, 0.75f - 0.5f * s, 0.1f + 0.1f * s}; // Gold to Amber-Red
    }
}

std::array<float, 3> cross(const std::array<float, 3>& a, const std::array<float, 3>& b) {
    return {a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]};
}

std::array<float, 3> normalize(const std::array<float, 3>& v) {
    const float len = std::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    if (len < 1e-6f) return {0.0f, 0.0f, 1.0f};
    return {v[0] / len, v[1] / len, v[2] / len};
}

} // anonymous namespace

nlohmann::json HandMeshPayload::toJson() const {
    nlohmann::json verts_arr = nlohmann::json::array();
    nlohmann::json norms_arr = nlohmann::json::array();
    nlohmann::json colors_arr = nlohmann::json::array();
    for (const auto& v : vertices) {
        verts_arr.push_back(v.position);
        norms_arr.push_back(v.normal);
        colors_arr.push_back(v.color);
    }
    nlohmann::json tris_arr = nlohmann::json::array();
    for (const auto& t : triangles) {
        tris_arr.push_back(t);
    }
    return {{"vertices", verts_arr},
            {"normals", norms_arr},
            {"colors", colors_arr},
            {"triangles", tris_arr},
            {"min_elevation_mm", min_elevation_mm},
            {"max_elevation_mm", max_elevation_mm}};
}

std::string HandMeshPayload::toWavefrontObj() const {
    std::ostringstream ss;
    ss << "# CurvEngine Hand 3D Topographic Mesh\n";
    for (const auto& v : vertices) {
        ss << "v " << v.position[0] << " " << v.position[1] << " " << v.position[2] << " " << v.color[0] << " "
           << v.color[1] << " " << v.color[2] << "\n";
    }
    for (const auto& v : vertices) {
        ss << "vn " << v.normal[0] << " " << v.normal[1] << " " << v.normal[2] << "\n";
    }
    for (const auto& t : triangles) {
        ss << "f " << (t[0] + 1) << "//" << (t[0] + 1) << " " << (t[1] + 1) << "//" << (t[1] + 1) << " " << (t[2] + 1)
           << "//" << (t[2] + 1) << "\n";
    }
    return ss.str();
}

HandMeshPayload HandMeshGenerator::generate(const std::vector<std::array<float, 2>>& lm,
                                            const std::unordered_map<std::string, float>& pad_heights,
                                            float palm_breadth_mm, int image_width, int image_height) {
    HandMeshPayload payload;
    if (lm.size() < 21) return payload;

    for (size_t i = 0; i < 21; ++i) {
        if (!std::isfinite(lm[i][0]) || !std::isfinite(lm[i][1])) {
            return payload;
        }
    }

    const float ref_breadth = palm_breadth_mm > 0.0f ? palm_breadth_mm : 80.0f;
    const float w = static_cast<float>(std::max(1, image_width));
    const float h = static_cast<float>(std::max(1, image_height));

    const float dx = (lm[17][0] - lm[5][0]) * w;
    const float dy = (lm[17][1] - lm[5][1]) * h;
    const float breadth_px = std::sqrt(dx * dx + dy * dy);
    if (!std::isfinite(breadth_px) || breadth_px <= 1e-4f) return payload;

    const float mm_per_px = ref_breadth / breadth_px;
    if (!std::isfinite(mm_per_px) || mm_per_px <= 0.0f) return payload;

    // Center on mid-palm (average of wrist and middle MCP)
    const float cx = 0.5f * (lm[0][0] + lm[9][0]);
    const float cy = 0.5f * (lm[0][1] + lm[9][1]);

    auto padH = [&](const std::string& name, const std::string& legacy_alt, float def_h) -> float {
        if (pad_heights.count(name) > 0) {
            float v = pad_heights.at(name);
            if (std::isfinite(v)) return v;
        }
        if (pad_heights.count(legacy_alt) > 0) {
            float v = pad_heights.at(legacy_alt);
            if (std::isfinite(v)) return v;
        }
        return def_h;
    };

    const float h_thenar = padH("thenar", "venus", 3.0f);
    const float h_hypothenar = padH("hypothenar", "luna", 2.0f);
    const float h_radial = padH("radial_interdigital", "jupiter", 1.8f);
    const float h_central = padH("central_interdigital", "saturn", 1.5f);
    const float h_ring = padH("ring_interdigital", "apollo", 1.6f);
    const float h_ulnar = padH("ulnar_interdigital", "mercury", 1.4f);
    const float h_fossa = padH("palmar_fossa", "mars", -1.5f);

    std::vector<float> z_vals(22, 0.0f);
    z_vals[0] = 0.0f; // Wrist
    z_vals[1] = h_thenar * 0.7f;
    z_vals[2] = h_thenar;
    z_vals[3] = h_thenar * 0.4f;
    z_vals[4] = 0.5f;

    z_vals[5] = h_radial;
    z_vals[6] = h_radial * 0.5f;
    z_vals[7] = 0.4f;
    z_vals[8] = 0.2f;

    z_vals[9] = h_central;
    z_vals[10] = h_central * 0.5f;
    z_vals[11] = 0.4f;
    z_vals[12] = 0.2f;

    z_vals[13] = h_ring;
    z_vals[14] = h_ring * 0.5f;
    z_vals[15] = 0.4f;
    z_vals[16] = 0.2f;

    z_vals[17] = h_hypothenar * 0.6f + h_ulnar * 0.4f;
    z_vals[18] = h_ulnar * 0.5f;
    z_vals[19] = 0.4f;
    z_vals[20] = 0.2f;

    z_vals[21] = h_fossa; // Palmar center fossa

    float min_z = z_vals[0];
    float max_z = z_vals[0];
    for (float z : z_vals) {
        min_z = std::min(min_z, z);
        max_z = std::max(max_z, z);
    }
    const float z_range = std::max(1e-3f, max_z - min_z);
    payload.min_elevation_mm = min_z;
    payload.max_elevation_mm = max_z;

    payload.vertices.resize(22);
    for (size_t i = 0; i < 21; ++i) {
        const float px = (lm[i][0] - cx) * w * mm_per_px;
        const float py = (cy - lm[i][1]) * h * mm_per_px; // y-up
        const float pz = z_vals[i];
        payload.vertices[i].position = {px, py, pz};
        payload.vertices[i].color = elevationColor((pz - min_z) / z_range);
        payload.vertices[i].normal = {0.0f, 0.0f, 1.0f};
    }

    // Fossa center vertex (21)
    const float fx = 0.0f;
    const float fy = 0.0f;
    payload.vertices[21].position = {fx, fy, z_vals[21]};
    payload.vertices[21].color = elevationColor((z_vals[21] - min_z) / z_range);
    payload.vertices[21].normal = {0.0f, 0.0f, 1.0f};

    // Palmar surface triangles connected to center fossa (21)
    static const int palmar_ring[8] = {0, 1, 2, 5, 9, 13, 17, 0};
    for (int i = 0; i < 7; ++i) {
        payload.triangles.push_back({21, palmar_ring[i], palmar_ring[i + 1]});
    }

    // Inter-MCP / interdigital web triangles (connecting adjacent digits)
    payload.triangles.push_back({2, 5, 21});
    payload.triangles.push_back({5, 6, 9});
    payload.triangles.push_back({6, 10, 9});
    payload.triangles.push_back({9, 10, 13});
    payload.triangles.push_back({10, 14, 13});
    payload.triangles.push_back({13, 14, 17});
    payload.triangles.push_back({14, 18, 17});

    // Intermediate interdigital webbing
    payload.triangles.push_back({6, 7, 10});
    payload.triangles.push_back({7, 11, 10});
    payload.triangles.push_back({10, 11, 14});
    payload.triangles.push_back({11, 15, 14});
    payload.triangles.push_back({14, 15, 18});
    payload.triangles.push_back({15, 19, 18});

    // Compute smoothed surface normals from face normals
    for (const auto& tri : payload.triangles) {
        const auto& p0 = payload.vertices[static_cast<size_t>(tri[0])].position;
        const auto& p1 = payload.vertices[static_cast<size_t>(tri[1])].position;
        const auto& p2 = payload.vertices[static_cast<size_t>(tri[2])].position;

        const std::array<float, 3> v01 = {p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]};
        const std::array<float, 3> v02 = {p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]};
        const auto fn = normalize(cross(v01, v02));

        for (int idx : tri) {
            const size_t uidx = static_cast<size_t>(idx);
            payload.vertices[uidx].normal[0] += fn[0];
            payload.vertices[uidx].normal[1] += fn[1];
            payload.vertices[uidx].normal[2] += fn[2];
        }
    }

    for (auto& v : payload.vertices) {
        v.normal = normalize(v.normal);
    }

    return payload;
}

} // namespace CurvEngine::surface
