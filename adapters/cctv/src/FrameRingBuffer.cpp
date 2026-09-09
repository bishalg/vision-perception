#include "curv/adapters/FrameRingBuffer.hpp"

namespace CurvEngine::adapters {

FrameRingBuffer::FrameRingBuffer(std::size_t capacity) : capacity_(capacity) {
    if (capacity_ > 0) {
        slots_.resize(capacity_);
    }
}

FrameRingBuffer::PushResult FrameRingBuffer::push(Frame frame) {
    const std::scoped_lock lock(mutex_);
    if (capacity_ == 0) {
        return PushResult::Failed;
    }

    const bool overwriting = (count_ == capacity_);
    slots_[head_] = std::move(frame);
    head_ = (head_ + 1) % capacity_;
    if (!overwriting) {
        ++count_;
    }
    return overwriting ? PushResult::Overwritten : PushResult::Inserted;
}

std::optional<Frame> FrameRingBuffer::takeLatest() {
    const std::scoped_lock lock(mutex_);
    if (count_ == 0) {
        return std::nullopt;
    }

    // Newest frame is at (head_ + capacity_ - 1) % capacity_
    const std::size_t newest = (head_ + capacity_ - 1) % capacity_;
    Frame out = std::move(slots_[newest]);
    slots_[newest] = Frame();
    count_ = 0;
    head_ = 0;
    return out;
}

std::size_t FrameRingBuffer::size() const {
    const std::scoped_lock lock(mutex_);
    return count_;
}

std::size_t FrameRingBuffer::capacity() const noexcept {
    return capacity_;
}

bool FrameRingBuffer::empty() const {
    const std::scoped_lock lock(mutex_);
    return count_ == 0;
}

void FrameRingBuffer::clear() {
    const std::scoped_lock lock(mutex_);
    for (auto& slot : slots_) {
        slot = Frame();
    }
    count_ = 0;
    head_ = 0;
}

} // namespace CurvEngine::adapters
