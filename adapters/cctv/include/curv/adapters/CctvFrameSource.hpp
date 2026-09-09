#pragma once

#include "curv/Frame.hpp"
#include "curv/QualityReport.hpp"
#include "curv/quality/LaplacianQualityAnalyzer.hpp"
#include <cstdint>
#include <string>
#include <vector>

namespace CurvEngine::adapters {

/**
 * @brief Telemetry, geodetic position, and mounting pose for a CCTV camera.
 */
struct CctvMetadata {
    std::string camera_id;
    std::string name;
    std::string agency;
    double latitude{0.0};
    double longitude{0.0};
    double elevation_m{0.0};
    double heading_deg{0.0};
    double pitch_deg{0.0};
    double roll_deg{0.0};
    double fov_horizontal_deg{60.0};
    std::string stream_or_snapshot_url;
};

/**
 * @brief Ingestion adapter for public CCTV camera snapshots and video frames.
 *
 * Provides safe frame decoding from filesystem paths or in-memory byte buffers,
 * metadata population, and integrated quality verification.
 */
class CctvFrameSource {
public:
    explicit CctvFrameSource(CctvMetadata metadata = {});
    ~CctvFrameSource() = default;

    [[nodiscard]] const CctvMetadata& getMetadata() const noexcept;
    void setMetadata(const CctvMetadata& metadata);

    /**
     * @brief Loads and decodes an image file into a Frame.
     * Checks for an accompanying sidecar .json metadata file to enrich telemetry.
     */
    [[nodiscard]] bool loadFromFile(const std::string& filepath, Frame& out_frame);

    /**
     * @brief Decodes a memory buffer (e.g. from network download) into a Frame.
     */
    [[nodiscard]] bool loadFromMemory(const std::vector<uint8_t>& buffer, Frame& out_frame);

    /**
     * @brief Evaluates frame suitability against the core Laplacian quality analyzer.
     */
    [[nodiscard]] QualityReport validateFrame(const Frame& frame, quality::LaplacianQualityAnalyzer& analyzer) const;

private:
    CctvMetadata metadata_;
    void populateFrameMetadata(Frame& frame) const;
    void parseSidecarJson(const std::string& json_path);
};

} // namespace CurvEngine::adapters
