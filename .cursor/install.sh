#!/usr/bin/env bash
# ==============================================================================
# Cloud Agent bootstrap for vision-perception.
#
# Idempotent: safe to re-run and safe when booting from a prepared snapshot
# (the heavy system-package step is skipped once the toolchain is present).
# ==============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "==> [1/4] Ensuring system toolchain & libraries..."
# Stable system dependencies for the C++ core (OpenCV, spdlog, nlohmann-json,
# GoogleTest, libcurl) plus ninja/clang-tidy. libstdc++-14-dev is required
# because the VM default `c++` alternative is Clang 18, which auto-selects the
# GCC 14 toolchain directory. Guarded so a prepared snapshot boots instantly.
if ! pkg-config --exists opencv4 || ! command -v ninja >/dev/null 2>&1; then
  # Retry/timeout options make the download resilient to flaky Ubuntu mirrors
  # (occasional 502s), so a single bad response doesn't fail the whole build.
  APT_OPTS=(-o Acquire::Retries=5 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30)
  sudo apt-get "${APT_OPTS[@]}" update
  sudo apt-get "${APT_OPTS[@]}" install -y --no-install-recommends \
    build-essential ninja-build libstdc++-14-dev \
    libopencv-dev nlohmann-json3-dev libspdlog-dev libgtest-dev \
    libcurl4-openssl-dev clang-tidy
fi

echo "==> [2/4] Ensuring pinned clang-format (18.1.8, matches CI)..."
if ! command -v clang-format >/dev/null 2>&1 \
   || [[ "$(clang-format --version 2>/dev/null)" != *"18.1.8"* ]]; then
  python3 -m pip install --user --break-system-packages --quiet clang-format==18.1.8
fi
# Expose the pip-installed clang-format on PATH for every shell/terminal.
if [[ -x "$HOME/.local/bin/clang-format" ]]; then
  sudo ln -sf "$HOME/.local/bin/clang-format" /usr/local/bin/clang-format
fi

echo "==> [3/4] Configuring & building the C++ core (Release, gcc)..."
# Match the Ubuntu CI job, which builds with gcc under -Werror. The VM's default
# `c++` alternative points at Clang, so pin gcc/g++ explicitly for a consistent
# warnings-as-errors build. Produces build/tools/{curv_cli,hand_inspector_cli}
# that the studio-api backend invokes for inference.
CC=gcc CXX=g++ cmake --preset release
cmake --build --preset release -j"$(nproc)"

echo "==> [4/4] Installing web application dependencies..."
( cd apps/annotation_studio && npm ci )
( cd apps/cesium_viewer && npm ci )

echo "==> vision-perception environment ready."
