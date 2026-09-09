#include "curv/adapters/CctvStreamAdapter.hpp"
#include <filesystem>
#include <opencv2/core.hpp>
#include <opencv2/videoio.hpp>
#include <utility>

namespace CurvEngine::adapters {

struct CctvStreamAdapter::CaptureState {
    cv::VideoCapture cap;
};

CctvStreamAdapter::CctvStreamAdapter(CctvMetadata metadata, std::size_t ring_capacity)
    : metadata_(std::move(metadata)), ring_(ring_capacity) {}

CctvStreamAdapter::~CctvStreamAdapter() {
    close();
}

const CctvMetadata& CctvStreamAdapter::getMetadata() const noexcept {
    return metadata_;
}

void CctvStreamAdapter::setMetadata(const CctvMetadata& metadata) {
    metadata_ = metadata;
}

bool CctvStreamAdapter::openCapture(const std::string& source, bool is_url) {
    stop();

    if (!is_url && !std::filesystem::exists(source)) {
        return false;
    }

    auto state = std::make_unique<CaptureState>();
    const bool opened = is_url ? state->cap.open(source, cv::CAP_FFMPEG) : state->cap.open(source);
    if (!opened || !state->cap.isOpened()) {
        return false;
    }

    capture_ = std::move(state);
    opened_.store(true, std::memory_order_release);
    frames_captured_.store(0, std::memory_order_relaxed);
    frames_overwritten_.store(0, std::memory_order_relaxed);
    sequence_.store(0, std::memory_order_relaxed);
    ring_.clear();
    return true;
}

bool CctvStreamAdapter::openFile(const std::string& filepath) {
    return openCapture(filepath, false);
}

bool CctvStreamAdapter::openUrl(const std::string& url) {
    if (url.empty()) {
        return false;
    }
    return openCapture(url, true);
}

void CctvStreamAdapter::close() noexcept {
    stop();
    opened_.store(false, std::memory_order_release);
    capture_.reset();
    ring_.clear();
}

bool CctvStreamAdapter::start() {
    if (!opened_.load(std::memory_order_acquire) || capture_ == nullptr) {
        return false;
    }
    if (running_.load(std::memory_order_acquire)) {
        return true;
    }

    stop_requested_.store(false, std::memory_order_release);
    try {
        worker_ = std::thread(&CctvStreamAdapter::captureLoop, this);
    } catch (...) {
        return false;
    }
    running_.store(true, std::memory_order_release);
    return true;
}

void CctvStreamAdapter::stop() noexcept {
    stop_requested_.store(true, std::memory_order_release);
    if (worker_.joinable()) {
        try {
            worker_.join();
        } catch (...) { // NOLINT(bugprone-empty-catch) — noexcept stop must absorb join failures
        }
    }
    running_.store(false, std::memory_order_release);
    frame_cv_.notify_all();
}

bool CctvStreamAdapter::isOpened() const noexcept {
    return opened_.load(std::memory_order_acquire);
}

bool CctvStreamAdapter::isRunning() const noexcept {
    return running_.load(std::memory_order_acquire);
}

void CctvStreamAdapter::setLoopPlayback(bool enabled) noexcept {
    loop_playback_.store(enabled, std::memory_order_relaxed);
}

bool CctvStreamAdapter::loopPlayback() const noexcept {
    return loop_playback_.load(std::memory_order_relaxed);
}

std::uint64_t CctvStreamAdapter::framesCaptured() const noexcept {
    return frames_captured_.load(std::memory_order_relaxed);
}

std::uint64_t CctvStreamAdapter::framesOverwritten() const noexcept {
    return frames_overwritten_.load(std::memory_order_relaxed);
}

void CctvStreamAdapter::populateFrameMetadata(Frame& frame, std::uint64_t sequence) const {
    frame.metadata["camera_id"] = metadata_.camera_id;
    frame.metadata["name"] = metadata_.name;
    frame.metadata["agency"] = metadata_.agency;
    frame.metadata["latitude"] = std::to_string(metadata_.latitude);
    frame.metadata["longitude"] = std::to_string(metadata_.longitude);
    frame.metadata["elevation_m"] = std::to_string(metadata_.elevation_m);
    frame.metadata["heading_deg"] = std::to_string(metadata_.heading_deg);
    frame.metadata["pitch_deg"] = std::to_string(metadata_.pitch_deg);
    frame.metadata["fov_horizontal_deg"] = std::to_string(metadata_.fov_horizontal_deg);
    frame.metadata["stream_sequence"] = std::to_string(sequence);
    if (!metadata_.stream_or_snapshot_url.empty()) {
        frame.metadata["stream_or_snapshot_url"] = metadata_.stream_or_snapshot_url;
    }
}

bool CctvStreamAdapter::injectFrame(Frame frame) {
    const std::uint64_t seq = sequence_.fetch_add(1, std::memory_order_relaxed) + 1;
    populateFrameMetadata(frame, seq);
    if (!metadata_.camera_id.empty()) {
        frame.source_id = metadata_.camera_id;
    } else if (frame.source_id.empty()) {
        frame.source_id = "cctv_stream";
    }
    frame.timestamp_ms = seq;

    const auto result = ring_.push(std::move(frame));
    if (result == FrameRingBuffer::PushResult::Failed) {
        return false;
    }
    if (result == FrameRingBuffer::PushResult::Overwritten) {
        frames_overwritten_.fetch_add(1, std::memory_order_relaxed);
    }
    frames_captured_.fetch_add(1, std::memory_order_relaxed);
    frame_cv_.notify_one();
    return true;
}

bool CctvStreamAdapter::tryLatestFrame(Frame& out_frame) {
    auto maybe = ring_.takeLatest();
    if (!maybe.has_value()) {
        return false;
    }
    out_frame = std::move(*maybe);
    return true;
}

bool CctvStreamAdapter::waitLatestFrame(Frame& out_frame, std::chrono::milliseconds timeout) {
    std::unique_lock<std::mutex> lock(state_mutex_);
    // Wake only on a new frame or an explicit stop — do not treat "not running"
    // as success, or inject-only waits return immediately with an empty ring.
    const bool signaled = frame_cv_.wait_for(
        lock, timeout, [this] { return !ring_.empty() || stop_requested_.load(std::memory_order_acquire); });
    lock.unlock();

    if (!signaled && ring_.empty()) {
        return false;
    }
    return tryLatestFrame(out_frame);
}

void CctvStreamAdapter::captureLoop() {
    while (!stop_requested_.load(std::memory_order_acquire)) {
        if (capture_ == nullptr || !capture_->cap.isOpened()) {
            break;
        }

        cv::Mat image;
        if (!capture_->cap.read(image) || image.empty()) {
            if (loop_playback_.load(std::memory_order_relaxed)) {
                // Rewind to first frame for continuous soak tests.
                capture_->cap.set(cv::CAP_PROP_POS_FRAMES, 0);
                continue;
            }
            break;
        }

        const std::uint64_t seq = sequence_.fetch_add(1, std::memory_order_relaxed) + 1;
        const std::string source_id = metadata_.camera_id.empty() ? "cctv_stream" : metadata_.camera_id;
        Frame frame(image, source_id, seq);
        populateFrameMetadata(frame, seq);

        const auto result = ring_.push(std::move(frame));
        if (result != FrameRingBuffer::PushResult::Failed) {
            if (result == FrameRingBuffer::PushResult::Overwritten) {
                frames_overwritten_.fetch_add(1, std::memory_order_relaxed);
            }
            frames_captured_.fetch_add(1, std::memory_order_relaxed);
            frame_cv_.notify_one();
        }
    }

    running_.store(false, std::memory_order_release);
    frame_cv_.notify_all();
}

} // namespace CurvEngine::adapters
