#pragma once

#include <array>
#include <nlohmann/json.hpp>
#include <string>
#include <unordered_map>
#include <vector>

namespace CurvEngine::surface {

struct MeshVertex {
    std::array<float, 3> position{0.0f, 0.0f, 0.0f}; // mm
    std::array<float, 3> normal{0.0f, 0.0f, 1.0f};
    std::array<float, 3> color{0.2f, 0.7f, 0.4f}; // RGB [0..1]
};

struct HandMeshPayload {
    std::vector<MeshVertex> vertices;
    std::vector<std::array<int, 3>> triangles;
    float min_elevation_mm{0.0f};
    float max_elevation_mm{0.0f};

    [[nodiscard]] nlohmann::json toJson() const;
    [[nodiscard]] std::string toWavefrontObj() const;
};

class HandMeshGenerator {
public:
    [[nodiscard]] static HandMeshPayload generate(const std::vector<std::array<float, 2>>& landmarks_21,
                                                  const std::unordered_map<std::string, float>& pad_heights_mm = {},
                                                  float palm_breadth_mm = 80.0f, int image_width = 1000,
                                                  int image_height = 1000);
};

} // namespace CurvEngine::surface
