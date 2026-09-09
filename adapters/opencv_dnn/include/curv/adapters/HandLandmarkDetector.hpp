#pragma once

#include "curv/geometry/RoiWarper.hpp"
#include <opencv2/core.hpp>
#include <opencv2/dnn.hpp>
#include <optional>
#include <string>

namespace CurvEngine::adapters {

/**
 * @brief Lightweight desktop hand landmark detector utilizing OpenCV's native DNN module.
 *
 * Supports reading standard open-source ONNX hand pose models (e.g., 21 landmark output),
 * and provides an automatic anatomical heuristic fallback for standalone testing.
 */
class HandLandmarkDetector {
public:
    explicit HandLandmarkDetector(const std::string& model_path = "");

    /**
     * @brief Loads an ONNX model file using cv::dnn::readNetFromONNX.
     * @return true if network was successfully initialized.
     */
    bool loadModel(const std::string& model_path);

    /**
     * @brief Checks if an ONNX model is currently loaded.
     */
    [[nodiscard]] bool isModelLoaded() const noexcept;

    /**
     * @brief Detects 21 hand landmarks from an input image.
     * If an ONNX model is loaded, runs forward inference and decodes 21 keypoints.
     * If no model is loaded, computes a robust heuristic anatomical palm landmark set.
     * @param image Input image (BGR, RGB, or Grayscale).
     * @return HandLandmarks struct containing 21 normalized coordinates.
     */
    [[nodiscard]] std::optional<geometry::HandLandmarks> detect(const cv::Mat& image);

    /**
     * @brief Generates robust anatomical baseline hand landmarks based on image dimensions.
     */
    static geometry::HandLandmarks generateHeuristicLandmarks(int width, int height, bool is_left_hand = false);

private:
    cv::dnn::Net net_;
    bool is_loaded_{false};
    int input_width_{224};
    int input_height_{224};
};

} // namespace CurvEngine::adapters
