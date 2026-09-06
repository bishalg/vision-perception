#!/usr/bin/env bash
# ==============================================================================
# scripts/run_flood_pipeline.sh
# Runs curv_cli on PRE and POST Sentinel-2 flood images with identical
# wide-ridge parameters and exports evidence JSON, overlay PNG, and vector SVG.
# ==============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

CLI_BIN="${ROOT_DIR}/build/tools/curv_cli"
DATA_DIR="${ROOT_DIR}/data/flood_nepal_2026"
PROC_DIR="${DATA_DIR}/processed"
OUT_DIR="${DATA_DIR}/out"

mkdir -p "${OUT_DIR}"

# Parameters from configs/geospatial_wide_ridge.json
SIGMA=5.0
LOW=0.2
HIGH=0.8
MIN_BLUR=15.0

echo "=================================================="
echo "Running Vision-Perception Flood Pipeline (C++20)"
echo "Config: sigma=${SIGMA}, low=${LOW}, high=${HIGH}, min_blur=${MIN_BLUR}"
echo "=================================================="

# 1. PRE Image (2026-08-24)
echo "[1/2] Processing PRE (2026-08-24)..."
"${CLI_BIN}" \
    --image "${PROC_DIR}/pre_20260824_bgr.png" \
    --sigma "${SIGMA}" \
    --low "${LOW}" \
    --high "${HIGH}" \
    --min-blur "${MIN_BLUR}" \
    --out "${OUT_DIR}/pre_overlay.png" \
    --svg "${OUT_DIR}/pre_curves.svg" \
    --json "${OUT_DIR}/pre_evidence.json"

# 2. POST Image (2026-08-27)
echo "[2/2] Processing POST (2026-08-27)..."
"${CLI_BIN}" \
    --image "${PROC_DIR}/post_20260827_bgr.png" \
    --sigma "${SIGMA}" \
    --low "${LOW}" \
    --high "${HIGH}" \
    --min-blur "${MIN_BLUR}" \
    --out "${OUT_DIR}/post_overlay.png" \
    --svg "${OUT_DIR}/post_curves.svg" \
    --json "${OUT_DIR}/post_evidence.json"

echo "[SUCCESS] Pipeline execution complete for PRE and POST."
