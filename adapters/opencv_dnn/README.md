# OpenCV-DNN Hand Landmark Detector

`HandLandmarkDetector` (`include/curv/adapters/HandLandmarkDetector.hpp`) —
21-keypoint hand landmark estimation via `cv::dnn` ONNX inference:

- Loads any compatible 21-landmark ONNX model
  (`tools/scripts/download_hand_model.py` fetches one; see `models/README.md`).
- Output coordinates are normalized to [0, 1] and clamped.
- **Heuristic fallback**: when no model is loaded or inference fails (logged via
  spdlog), an anatomical MediaPipe-topology palm estimate is produced instead —
  the pipeline never hard-fails for lack of a model.

Used by `hand_inspector_cli --auto-landmarks` and the mobile landmark-ROI path.
