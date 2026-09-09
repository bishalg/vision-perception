#include "curv/adapters/CctvVideoSource.hpp"
#include <algorithm>
#include <filesystem>
#include <opencv2/core.hpp>
#include <opencv2/videoio.hpp>
#include <utility>

namespace CurvEngine::adapters {

struct CctvVideoSource::CaptureState {
    cv::VideoCapture cap;
};

CctvVideoSource::CctvVideoSource(CctvMetadata metadata) : metadata_(std::move(metadata)) {}

CctvVideoSource::~CctvVideoSource() {
    close();
}

const CctvMetadata& CctvVideoSource::getMetadata() const noexcept {
    return metadata_;
}

void CctvVideoSource::setMetadata(const CctvMetadata& metadata) {
    metadata_ = metadata;
}

bool CctvVideoSource::open(const std::string& filepath) {
    close();

    if (!std::filesystem::exists(filepath)) {
        return false;
    }

    auto state = std::make_unique<CaptureState>();
    if (!state->cap.open(filepath) || !state->cap.isOpened()) {
        return false;
    }

    capture_ = std::move(state);
    frames_decoded_ = 0;
    frames_emitted_ = 0;
    opened_ = true;
    return true;
}

void CctvVideoSource::close() noexcept {
    capture_.reset();
    opened_ = false;
}

bool CctvVideoSource::isOpened() const noexcept {
    return opened_ && capture_ != nullptr && capture_->cap.isOpened();
}

void CctvVideoSource::setFpsStride(int stride) noexcept {
    fps_stride_ = std::max(1, stride);
}

int CctvVideoSource::fpsStride() const noexcept {
    return fps_stride_;
}

double CctvVideoSource::sourceFps() const noexcept {
    if (!isOpened()) {
        return 0.0;
    }
    return capture_->cap.get(cv::CAP_PROP_FPS);
}

std::int64_t CctvVideoSource::frameCount() const noexcept {
    if (!isOpened()) {
        return 0;
    }
    return static_cast<std::int64_t>(capture_->cap.get(cv::CAP_PROP_FRAME_COUNT));
}

std::uint64_t CctvVideoSource::framesEmitted() const noexcept {
    return frames_emitted_;
}

std::uint64_t CctvVideoSource::framesDecoded() const noexcept {
    return frames_decoded_;
}

void CctvVideoSource::populateFrameMetadata(Frame& frame, std::uint64_t sequence) const {
    frame.metadata["camera_id"] = metadata_.camera_id;
    frame.metadata["name"] = metadata_.name;
    frame.metadata["agency"] = metadata_.agency;
    frame.metadata["latitude"] = std::to_string(metadata_.latitude);
    frame.metadata["longitude"] = std::to_string(metadata_.longitude);
    frame.metadata["elevation_m"] = std::to_string(metadata_.elevation_m);
    frame.metadata["heading_deg"] = std::to_string(metadata_.heading_deg);
    frame.metadata["pitch_deg"] = std::to_string(metadata_.pitch_deg);
    frame.metadata["fov_horizontal_deg"] = std::to_string(metadata_.fov_horizontal_deg);
    frame.metadata["video_sequence"] = std::to_string(sequence);
    frame.metadata["fps_stride"] = std::to_string(fps_stride_);
}

bool CctvVideoSource::readNext(Frame& out_frame) {
    if (!isOpened()) {
        out_frame = Frame();
        return false;
    }

    cv::Mat image;
    while (true) {
        if (!capture_->cap.read(image) || image.empty()) {
            out_frame = Frame();
            return false;
        }

        ++frames_decoded_;

        // Stride: emit when decoded index (1-based) hits every Nth frame.
        if (((frames_decoded_ - 1) % static_cast<std::uint64_t>(fps_stride_)) == 0) {
            break;
        }
    }

    ++frames_emitted_;
    const std::string source_id = metadata_.camera_id.empty() ? "cctv_video" : metadata_.camera_id;
    out_frame = Frame(image, source_id, frames_emitted_);
    populateFrameMetadata(out_frame, frames_emitted_);
    return true;
}

} // namespace CurvEngine::adapters
