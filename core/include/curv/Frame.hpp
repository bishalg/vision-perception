#pragma once

#include <cstdint>
#include <opencv2/core.hpp>
#include <string>
#include <unordered_map>

namespace CurvEngine {

/**
 * @brief Represents an ingested image frame with associated capture telemetry and metadata.
 */
struct Frame {
    cv::Mat image;
    std::string source_id;
    uint64_t timestamp_ms{0};
    std::unordered_map<std::string, std::string> metadata;

    Frame() = default;

    explicit Frame(cv::Mat in_image, std::string in_source_id = "", uint64_t in_timestamp_ms = 0,
                   std::unordered_map<std::string, std::string> in_metadata = {});

    [[nodiscard]] bool empty() const noexcept;
    [[nodiscard]] int width() const noexcept;
    [[nodiscard]] int height() const noexcept;
    [[nodiscard]] int channels() const noexcept;
};

} // namespace CurvEngine
