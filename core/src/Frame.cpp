#include "curv/Frame.hpp"

namespace CurvEngine {

Frame::Frame(cv::Mat in_image, std::string in_source_id, uint64_t in_timestamp_ms,
             std::unordered_map<std::string, std::string> in_metadata)
    : image(std::move(in_image)), source_id(std::move(in_source_id)), timestamp_ms(in_timestamp_ms),
      metadata(std::move(in_metadata)) {}

bool Frame::empty() const noexcept {
    return image.empty();
}

int Frame::width() const noexcept {
    return image.cols;
}

int Frame::height() const noexcept {
    return image.rows;
}

int Frame::channels() const noexcept {
    return image.channels();
}

} // namespace CurvEngine
