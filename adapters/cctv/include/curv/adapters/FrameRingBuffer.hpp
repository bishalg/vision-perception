#pragma once

#include "curv/Frame.hpp"
#include <cstddef>
#include <cstdint>
#include <mutex>
#include <optional>
#include <vector>

namespace CurvEngine::adapters {

/**
 * @brief Fixed-capacity thread-safe ring buffer for CCTV frames.
 *
 * Producer threads push frames; when full, the oldest slot is overwritten
 * (backpressure via drop-oldest). Consumers take the newest unread frame and
 * discard stale intermediates (zero-latency accumulation).
 */
class FrameRingBuffer {
public:
    explicit FrameRingBuffer(std::size_t capacity = 16);

    FrameRingBuffer(const FrameRingBuffer&) = delete;
    FrameRingBuffer& operator=(const FrameRingBuffer&) = delete;

    enum class PushResult : std::uint8_t {
        Failed = 0,
        Inserted = 1,
        Overwritten = 2,
    };

    /**
     * @brief Insert a frame. Overwrites the oldest entry when at capacity.
     */
    [[nodiscard]] PushResult push(Frame frame);

    /**
     * @brief Pop the newest frame and clear all pending older frames.
     * @return nullopt when empty.
     */
    [[nodiscard]] std::optional<Frame> takeLatest();

    [[nodiscard]] std::size_t size() const;
    [[nodiscard]] std::size_t capacity() const noexcept;
    [[nodiscard]] bool empty() const;
    void clear();

private:
    mutable std::mutex mutex_;
    std::vector<Frame> slots_;
    std::size_t capacity_{0};
    std::size_t head_{0};  // next write index
    std::size_t count_{0}; // occupied slots
};

} // namespace CurvEngine::adapters
