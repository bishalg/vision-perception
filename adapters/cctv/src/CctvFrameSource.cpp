#include "curv/adapters/CctvFrameSource.hpp"
#include <filesystem>
#include <fstream>
#include <nlohmann/json.hpp>
#include <opencv2/imgcodecs.hpp>

namespace CurvEngine::adapters {

CctvFrameSource::CctvFrameSource(CctvMetadata metadata) : metadata_(std::move(metadata)) {}

const CctvMetadata& CctvFrameSource::getMetadata() const noexcept {
    return metadata_;
}

void CctvFrameSource::setMetadata(const CctvMetadata& metadata) {
    metadata_ = metadata;
}

void CctvFrameSource::parseSidecarJson(const std::string& json_path) {
    if (!std::filesystem::exists(json_path)) {
        return;
    }

    try {
        std::ifstream f(json_path);
        if (!f.is_open()) {
            return;
        }

        nlohmann::json j;
        f >> j;

        if (j.contains("agency") && j["agency"].is_string()) {
            metadata_.agency = j["agency"].get<std::string>();
        }
        if (j.contains("name") && j["name"].is_string()) {
            metadata_.name = j["name"].get<std::string>();
        }
        if (j.contains("location") && j["location"].is_object()) {
            const auto& loc = j["location"];
            if (loc.contains("lat") && loc["lat"].is_number()) {
                metadata_.latitude = loc["lat"].get<double>();
            }
            if (loc.contains("lon") && loc["lon"].is_number()) {
                metadata_.longitude = loc["lon"].get<double>();
            }
            if (loc.contains("elevation_m") && loc["elevation_m"].is_number()) {
                metadata_.elevation_m = loc["elevation_m"].get<double>();
            }
        }
        if (j.contains("pose") && j["pose"].is_object()) {
            const auto& pose = j["pose"];
            if (pose.contains("heading_deg") && pose["heading_deg"].is_number()) {
                metadata_.heading_deg = pose["heading_deg"].get<double>();
            }
            if (pose.contains("pitch_deg") && pose["pitch_deg"].is_number()) {
                metadata_.pitch_deg = pose["pitch_deg"].get<double>();
            }
            if (pose.contains("fov_deg") && pose["fov_deg"].is_number()) {
                metadata_.fov_horizontal_deg = pose["fov_deg"].get<double>();
            }
        }
    } catch (...) { // NOLINT(bugprone-empty-catch) — intentional resilience boundary: sidecar JSON is optional;
                    // partial/default metadata is safe
    }
}

bool CctvFrameSource::loadFromFile(const std::string& filepath, Frame& out_frame) {
    if (!std::filesystem::exists(filepath)) {
        out_frame = Frame();
        return false;
    }

    const cv::Mat img = cv::imread(filepath, cv::IMREAD_COLOR);
    if (img.empty()) {
        out_frame = Frame();
        return false;
    }

    // Auto-detect and parse sidecar JSON
    std::filesystem::path p(filepath);
    const std::string json_path = p.replace_extension(".json").string();
    parseSidecarJson(json_path);

    const std::string source_id = metadata_.camera_id.empty() ? p.stem().string() : metadata_.camera_id;
    out_frame = Frame(img, source_id);
    populateFrameMetadata(out_frame);

    return true;
}

bool CctvFrameSource::loadFromMemory(const std::vector<uint8_t>& buffer, Frame& out_frame) {
    if (buffer.empty()) {
        out_frame = Frame();
        return false;
    }

    const cv::Mat img = cv::imdecode(buffer, cv::IMREAD_COLOR);
    if (img.empty()) {
        out_frame = Frame();
        return false;
    }

    const std::string source_id = metadata_.camera_id.empty() ? "cctv_stream_buffer" : metadata_.camera_id;
    out_frame = Frame(img, source_id);
    populateFrameMetadata(out_frame);

    return true;
}

void CctvFrameSource::populateFrameMetadata(Frame& frame) const {
    frame.metadata["camera_id"] = metadata_.camera_id;
    frame.metadata["name"] = metadata_.name;
    frame.metadata["agency"] = metadata_.agency;
    frame.metadata["latitude"] = std::to_string(metadata_.latitude);
    frame.metadata["longitude"] = std::to_string(metadata_.longitude);
    frame.metadata["elevation_m"] = std::to_string(metadata_.elevation_m);
    frame.metadata["heading_deg"] = std::to_string(metadata_.heading_deg);
    frame.metadata["pitch_deg"] = std::to_string(metadata_.pitch_deg);
    frame.metadata["fov_horizontal_deg"] = std::to_string(metadata_.fov_horizontal_deg);
}

// NOLINTNEXTLINE(readability-convert-member-functions-to-static) — keep instance API for future override
QualityReport CctvFrameSource::validateFrame(const Frame& frame, quality::LaplacianQualityAnalyzer& analyzer) const {
    return analyzer.analyze(frame);
}

} // namespace CurvEngine::adapters
