#pragma once

#include "curv/Frame.hpp"
#include "curv/adapters/CctvFrameSource.hpp"
#include "curv/adapters/FrameRingBuffer.hpp"
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <memory>
#include <mutex>
#include <string>
#include <thread>

namespace CurvEngine::adapters {

/**
 * @brief Offline / live CCTV video ingestion with a thread-safe ring buffer.
 *
 * Stage 2 targets recorded local files (mp4/avi) for deterministic CI.
 * Stage 3 may open RTSP/HLS URLs via the same OpenCV VideoCapture path.
 *
 * Capture runs on a background thread; the perception thread consumes via
 * tryLatestFrame / waitLatestFrame, always receiving the newest frame.
 */
class CctvStreamAdapter {
public:
    explicit CctvStreamAdapter(CctvMetadata metadata = {}, std::size_t ring_capacity = 16);
    ~CctvStreamAdapter();

    CctvStreamAdapter(const CctvStreamAdapter&) = delete;
    CctvStreamAdapter& operator=(const CctvStreamAdapter&) = delete;

    [[nodiscard]] const CctvMetadata& getMetadata() const noexcept;
    void setMetadata(const CctvMetadata& metadata);

    /**
     * @brief Open a local recorded video file (Stage 2).
     * Does not start capture until start() is called.
     */
    [[nodiscard]] bool openFile(const std::string& filepath);

    /**
     * @brief Open a stream URL (RTSP/HLS/HTTP). Used in Stage 3; available for local tests.
     */
    [[nodiscard]] bool openUrl(const std::string& url);

    /**
     * @brief Close the capture device and stop the worker if running.
     */
    void close() noexcept;

    /**
     * @brief Start the background capture loop. Requires a successful open*.
     */
    [[nodiscard]] bool start();

    /**
     * @brief Stop the capture loop and join the worker thread.
     */
    void stop() noexcept;

    [[nodiscard]] bool isOpened() const noexcept;
    [[nodiscard]] bool isRunning() const noexcept;

    /**
     * @brief Non-blocking: take newest buffered frame (drops stale intermediates).
     */
    [[nodiscard]] bool tryLatestFrame(Frame& out_frame);

    /**
     * @brief Block until a frame is available or timeout elapses.
     */
    [[nodiscard]] bool waitLatestFrame(Frame& out_frame, std::chrono::milliseconds timeout);

    /**
     * @brief Inject a frame into the ring (unit tests / synthetic producers).
     * Does not require VideoCapture to be open.
     */
    [[nodiscard]] bool injectFrame(Frame frame);

    /**
     * @brief When true, file playback rewinds at EOF (continuous soak tests).
     * Default false: capture stops after the last frame.
     */
    void setLoopPlayback(bool enabled) noexcept;
    [[nodiscard]] bool loopPlayback() const noexcept;

    /**
     * @brief Frames successfully pushed by the capture worker (monotonic).
     */
    [[nodiscard]] std::uint64_t framesCaptured() const noexcept;

    /**
     * @brief Frames dropped because the ring was full at push time (overwrite count).
     */
    [[nodiscard]] std::uint64_t framesOverwritten() const noexcept;

private:
    void captureLoop();
    void populateFrameMetadata(Frame& frame, std::uint64_t sequence) const;
    [[nodiscard]] bool openCapture(const std::string& source, bool is_url);

    CctvMetadata metadata_;
    FrameRingBuffer ring_;

    // Opaque OpenCV VideoCapture; definition lives in the .cpp to avoid videoio in the header.
    struct CaptureState;
    std::unique_ptr<CaptureState> capture_;

    std::thread worker_;
    std::mutex state_mutex_;
    std::condition_variable frame_cv_;

    std::atomic<bool> opened_{false};
    std::atomic<bool> running_{false};
    std::atomic<bool> stop_requested_{false};
    std::atomic<bool> loop_playback_{false};
    std::atomic<std::uint64_t> frames_captured_{0};
    std::atomic<std::uint64_t> frames_overwritten_{0};
    std::atomic<std::uint64_t> sequence_{0};
};

} // namespace CurvEngine::adapters
