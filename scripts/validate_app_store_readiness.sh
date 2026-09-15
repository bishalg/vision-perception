#!/usr/bin/env bash
# =============================================================================
# App Store Readiness Validation Script for Vision-Perception Apps:
# 1. PalmReader (apps/ios_palm_app)
# 2. EarlyGaze (apps/ios_asd_gaze_app)
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "================================================================="
echo "  Validating App Store Fastlane & Metadata Readiness"
echo "================================================================="
echo ""

# 1. PalmReader
echo ">>> [1/2] Validating PalmReader (apps/ios_palm_app)..."
cd "$ROOT_DIR/apps/ios_palm_app/fastlane"
fastlane validate_metadata
echo "✅ PalmReader is fully App Store ready!"
echo ""

# 2. EarlyGaze
echo ">>> [2/2] Validating EarlyGaze (apps/ios_asd_gaze_app)..."
cd "$ROOT_DIR/apps/ios_asd_gaze_app/fastlane"
fastlane validate_metadata
echo "✅ EarlyGaze is fully App Store ready!"
echo ""

echo "================================================================="
echo "  🎉 All Vision-Perception iOS apps passed App Store validation!"
echo "================================================================="
