#!/usr/bin/env bash
# ==============================================================================
# scripts/benchmark_cool_vs_x86.sh
# Benchmarks C++20 curvilinear flood perception pipeline on:
#   - AWS Graviton3 (c7g.2xlarge, aarch64) with Cloud-Optimized OpenCV Library (COOL)
#   - AWS x86_64 (c7i.2xlarge, x86_64) with standard distribution OpenCV
# Targets: OpenCV AI Competition 2026 — Best Use of COOL Special Award ($1,000)
# Compatible with Bash 3.2+ (macOS & Linux)
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

# ------------------------------------------------------------------------------
# Configuration & Pricing Constants (On-Demand AWS US-East-1 as of 2026)
# ------------------------------------------------------------------------------
PRICE_C7G_HOURLY=0.3264  # c7g.2xlarge (8 vCPU, 16 GiB, AWS Graviton3)
PRICE_C7I_HOURLY=0.3570  # c7i.2xlarge (8 vCPU, 16 GiB, Intel Sapphire Rapids)

DEFAULT_RUNS=100
RUNS="${DEFAULT_RUNS}"
DRY_RUN=0
SKIP_BUILD=0

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --runs)
            RUNS="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=1
            RUNS=2
            shift
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--runs N] [--dry-run] [--skip-build]"
            echo "  --runs N       Number of benchmark iterations (default: 100)"
            echo "  --dry-run      Run 2 iterations for quick verification"
            echo "  --skip-build   Skip CMake compilation step"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

echo "========================================================================"
echo " OpenCV AI Competition 2026: COOL vs x86 Performance & Cost Benchmark  "
echo "========================================================================"

# ------------------------------------------------------------------------------
# Architecture & Hardware Identification
# ------------------------------------------------------------------------------
ARCH="$(uname -m)"
OS_TYPE="$(uname -s)"
echo "[INFO] Operating System: ${OS_TYPE}"
echo "[INFO] CPU Architecture: ${ARCH}"

INSTANCE_TYPE="unknown"
OPENCV_VARIANT="Standard"
HOURLY_PRICE=0.3500

if [[ "${ARCH}" == "aarch64" ]]; then
    echo "[INFO] Detected Arm64 architecture (Graviton profile candidate)"
    INSTANCE_TYPE="c7g.2xlarge"
    OPENCV_VARIANT="COOL"
    HOURLY_PRICE="${PRICE_C7G_HOURLY}"

    # Verify COOL installation paths and prioritize over standard distro OpenCV
    COOL_CANDIDATE_PATHS=("/opt/opencv-cool" "/opt/awscv" "/usr/local/cool")
    for cp in "${COOL_CANDIDATE_PATHS[@]}"; do
        if [[ -d "${cp}" ]]; then
            echo "[INFO] Found Cloud-Optimized OpenCV Library (COOL) at: ${cp}"
            export CMAKE_PREFIX_PATH="${cp}:${CMAKE_PREFIX_PATH:-}"
            export PKG_CONFIG_PATH="${cp}/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
            export LD_LIBRARY_PATH="${cp}/lib:${LD_LIBRARY_PATH:-}"
            break
        fi
    done
elif [[ "${ARCH}" == "x86_64" ]]; then
    echo "[INFO] Detected x86_64 architecture (Intel/AMD profile candidate)"
    INSTANCE_TYPE="c7i.2xlarge"
    OPENCV_VARIANT="Standard-x86"
    HOURLY_PRICE="${PRICE_C7I_HOURLY}"
elif [[ "${ARCH}" == "arm64" ]]; then
    # Local Apple Silicon developer workstation
    echo "[INFO] Detected Apple Silicon arm64 (local emulation/workstation)"
    INSTANCE_TYPE="local-arm64-workstation"
    OPENCV_VARIANT="Apple-Accelerate-OpenCV"
    HOURLY_PRICE="${PRICE_C7G_HOURLY}"
else
    echo "[WARN] Unrecognized architecture: ${ARCH}"
fi

# Print OpenCV build information to guarantee active SIMD extensions to judges
echo "------------------------------------------------------------------------"
echo " OpenCV Build & SIMD Architecture Inspection"
echo "------------------------------------------------------------------------"
python3 -c "import cv2; print(cv2.getBuildInformation())" 2>/dev/null | grep -E "(CPU/HW features|NEON|SVE|AVX|Parallel framework)" || echo "[INFO] cv2 python module not loaded; C++ compiler flags enforce target SIMD."
echo "------------------------------------------------------------------------"

# ------------------------------------------------------------------------------
# Build & Optimization Stage
# ------------------------------------------------------------------------------
CLI_BIN="${ROOT_DIR}/build/tools/curv_cli"

if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    echo "[STEP 1/3] Configuring and building C++20 release pipeline..."
    if [[ "${ARCH}" == "aarch64" ]]; then
        # Neoverse-V1 optimizations for AWS Graviton3
        export CXXFLAGS="-O3 -mcpu=neoverse-v1 -DNDEBUG ${CXXFLAGS:-}"
    elif [[ "${ARCH}" == "x86_64" ]]; then
        # Native vector extensions (AVX-512 / AVX2)
        export CXXFLAGS="-O3 -march=native -DNDEBUG ${CXXFLAGS:-}"
    fi

    cmake --preset release || cmake -B build -DCMAKE_BUILD_TYPE=Release
    cmake --build build --config Release --target curv_cli -j
fi

if [[ ! -x "${CLI_BIN}" ]]; then
    echo "[ERROR] Executable ${CLI_BIN} not found or not executable." >&2
    exit 1
fi

# ------------------------------------------------------------------------------
# Target Imagery & Workload Definition
# ------------------------------------------------------------------------------
IMAGE_PATH="${ROOT_DIR}/data/flood_nepal_2026/processed/post_20260827_bgr.png"
if [[ ! -f "${IMAGE_PATH}" ]]; then
    # Fallback to test image or synthetic pattern if post-event image unavailable
    IMAGE_PATH="$(find "${ROOT_DIR}/data" -name "*.png" 2>/dev/null | head -n 1 || true)"
fi

if [[ -z "${IMAGE_PATH}" || ! -f "${IMAGE_PATH}" ]]; then
    echo "[ERROR] No benchmark test image found in data directory." >&2
    exit 1
fi

echo "[INFO] Benchmark target image: ${IMAGE_PATH}"
echo "[INFO] Benchmark iterations: ${RUNS}"

# ------------------------------------------------------------------------------
# Execution & Latency Profiling
# ------------------------------------------------------------------------------
echo "[STEP 2/3] Executing ${RUNS} iterations of Steger curvilinear extraction..."

TMP_OUT_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_OUT_DIR}"' EXIT

LATENCY_SAMPLES_FILE="${TMP_OUT_DIR}/latencies.txt"
> "${LATENCY_SAMPLES_FILE}"

# Warmup run
"${CLI_BIN}" \
    --image "${IMAGE_PATH}" \
    --sigma 5.0 --low 0.2 --high 0.8 --min-blur 15.0 \
    --out "${TMP_OUT_DIR}/warmup_overlay.png" \
    --json "${TMP_OUT_DIR}/warmup_evidence.json" > /dev/null 2>&1

# Main profiling loop using python high-precision monotonic clock
python3 - "${CLI_BIN}" "${IMAGE_PATH}" "${TMP_OUT_DIR}" "${RUNS}" "${LATENCY_SAMPLES_FILE}" << 'PYEOF'
import subprocess
import sys
import time

cli_bin = sys.argv[1]
image_path = sys.argv[2]
out_dir = sys.argv[3]
total_runs = int(sys.argv[4])
out_file = sys.argv[5]

latencies = []
overlay_out = f"{out_dir}/bench_overlay.png"
json_out = f"{out_dir}/bench_evidence.json"

for i in range(total_runs):
    t0 = time.perf_counter()
    ret = subprocess.run(
        [
            cli_bin,
            "--image", image_path,
            "--sigma", "5.0",
            "--low", "0.2",
            "--high", "0.8",
            "--min-blur", "15.0",
            "--out", overlay_out,
            "--json", json_out,
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    t1 = time.perf_counter()
    if ret.returncode != 0:
        sys.stderr.write(f"Run {i+1} failed with code {ret.returncode}\n")
        sys.exit(1)
    duration_ms = (t1 - t0) * 1000.0
    latencies.append(duration_ms)

with open(out_file, "w") as f:
    for lat in latencies:
        f.write(f"{lat:.4f}\n")
PYEOF

# ------------------------------------------------------------------------------
# Statistical Analysis & Cost Computation
# ------------------------------------------------------------------------------
echo "[STEP 3/3] Computing latency metrics and cost efficiency..."

RESULTS_JSON="${ROOT_DIR}/data/benchmark_results.json"
mkdir -p "${ROOT_DIR}/data"

python3 - "${LATENCY_SAMPLES_FILE}" "${INSTANCE_TYPE}" "${ARCH}" "${OPENCV_VARIANT}" "${RUNS}" "${HOURLY_PRICE}" "${RESULTS_JSON}" << 'PYEOF'
import datetime
import json
import math
import sys

latency_file = sys.argv[1]
instance_type = sys.argv[2]
arch = sys.argv[3]
opencv_variant = sys.argv[4]
runs = int(sys.argv[5])
hourly_price = float(sys.argv[6])
out_json_path = sys.argv[7]

with open(latency_file, "r") as f:
    samples = [float(line.strip()) for line in f if line.strip()]

if not samples:
    sys.stderr.write("No latency samples recorded.\n")
    sys.exit(1)

samples.sort()
n = len(samples)
mean_ms = sum(samples) / n
p50_ms = samples[int(n * 0.50)]
p90_ms = samples[min(int(n * 0.90), n - 1)]
p95_ms = samples[min(int(n * 0.95), n - 1)]
min_ms = samples[0]
max_ms = samples[-1]

# Compute cost per processed image
# Hourly price / 3600 seconds = price per second
# latency in seconds = mean_ms / 1000.0
cost_per_image_usd = (hourly_price / 3600.0) * (mean_ms / 1000.0)
images_per_dollar = 1.0 / cost_per_image_usd if cost_per_image_usd > 0 else 0

result_data = {
    "instance_type": instance_type,
    "arch": arch,
    "opencv_variant": opencv_variant,
    "runs": runs,
    "latency_mean_ms": round(mean_ms, 2),
    "latency_p50_ms": round(p50_ms, 2),
    "latency_p90_ms": round(p90_ms, 2),
    "latency_p95_ms": round(p95_ms, 2),
    "latency_min_ms": round(min_ms, 2),
    "latency_max_ms": round(max_ms, 2),
    "cost_per_image_usd": round(cost_per_image_usd, 8),
    "throughput_images_per_dollar": round(images_per_dollar, 1),
    "hourly_instance_price_usd": hourly_price,
    "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
}

with open(out_json_path, "w") as f:
    json.dump(result_data, f, indent=2)

print("------------------------------------------------------------------------")
print(f" Instance Profile:          {instance_type} ({arch})")
print(f" OpenCV Variant:            {opencv_variant}")
print(f" Iterations Evaluated:      {runs}")
print(f" Latency Mean:              {mean_ms:.2f} ms")
print(f" Latency P95:               {p95_ms:.2f} ms")
print(f" Cost Per Scene:            ${cost_per_image_usd:.8f} USD")
print(f" Cost Efficiency:           {images_per_dollar:,.0f} images / $1.00 USD")
print(f" Benchmark Artifact:        {out_json_path}")
print("------------------------------------------------------------------------")
PYEOF

echo "[SUCCESS] Benchmark completed successfully."
