#pragma once

#include "curv/Frame.hpp"
#include "curv/adapters/CctvFrameSource.hpp"
#include <cstdint>
#include <memory>
#include <string>

namespace CurvEngine::adapters {

/**
 * @brief Phase 2 recorded-video ingestion: sequential frames via OpenCV VideoCapture.
 *
 * Emits Frame objects at a configurable FPS stride (process every Nth decoded frame).
 * Deterministic offline path for CI and presentation demos — no network required.
 */
class CctvVideoSource {
public:
    explicit CctvVideoSource(CctvMetadata metadata = {});
    ~CctvVideoSource();

    CctvVideoSource(const CctvVideoSource&) = delete;
    CctvVideoSource& operator=(const CctvVideoSource&) = delete;

    [[nodiscard]] const CctvMetadata& getMetadata() const noexcept;
    void setMetadata(const CctvMetadata& metadata);

    /**
     * @brief Open a local MP4/AVI/MKV recording.
     */
    [[nodiscard]] bool open(const std::string& filepath);

    void close() noexcept;

    [[nodiscard]] bool isOpened() const noexcept;

    /**
     * @brief Process every Nth decoded frame (N >= 1). Default 1 = every frame.
     */
    void setFpsStride(int stride) noexcept;
    [[nodiscard]] int fpsStride() const noexcept;

    /**
     * @brief Read the next strided frame into out_frame.
     * @return false on EOF or if not opened.
     */
    [[nodiscard]] bool readNext(Frame& out_frame);

    [[nodiscard]] double sourceFps() const noexcept;
    [[nodiscard]] std::int64_t frameCount() const noexcept;
    [[nodiscard]] std::uint64_t framesEmitted() const noexcept;
    [[nodiscard]] std::uint64_t framesDecoded() const noexcept;

private:
    void populateFrameMetadata(Frame& frame, std::uint64_t sequence) const;

    struct CaptureState;
    std::unique_ptr<CaptureState> capture_;

    CctvMetadata metadata_;
    int fps_stride_{1};
    std::uint64_t frames_decoded_{0};
    std::uint64_t frames_emitted_{0};
    bool opened_{false};
};

} // namespace CurvEngine::adapters
