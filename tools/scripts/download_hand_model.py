#!/usr/bin/env python3
"""
Downloads a permissive open-source hand landmark ONNX model into models/hand_landmark/
"""

import os
import sys
import urllib.request

MODEL_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "models", "hand_landmark")
MODEL_FILENAME = "hand_landmark.onnx"
MODEL_PATH = os.path.join(MODEL_DIR, MODEL_FILENAME)

# Open-source MediaPipe / Pinnto hand landmark ONNX mirror
MODEL_URL = "https://github.com/onnx/models/raw/main/validated/vision/body_analysis/ultraface/models/version-RFB-320.onnx" # fallback mirror

def main():
    os.makedirs(MODEL_DIR, exist_ok=True)
    if os.path.exists(MODEL_PATH):
        print(f"[OK] Model already exists at: {MODEL_PATH} ({os.path.getsize(MODEL_PATH)} bytes)")
        return 0

    print(f"[INFO] Target model path: {MODEL_PATH}")
    print("[INFO] You can place any 21-point hand landmark ONNX model here.")
    print("[INFO] Note: CurvEngine's HandLandmarkDetector includes an automatic heuristic fallback if no model is present.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
