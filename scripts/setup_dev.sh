#!/usr/bin/env bash
# ==============================================================================
# Developer Environment Setup Script
# ==============================================================================
set -euo pipefail

echo "==> Setting up developer environment for vision-perception..."

OS_TYPE="$(uname -s)"
case "${OS_TYPE}" in
    Darwin*)
        echo "Detected macOS environment."
        if command -v brew >/dev/null 2>&1; then
            echo "Checking Homebrew packages (cmake, ninja, opencv)..."
            brew install cmake ninja pkg-config || true
            # Optional: brew install opencv
        else
            echo "Homebrew not found. Please install Homebrew from https://brew.sh/"
        fi
        ;;
    Linux*)
        echo "Detected Linux environment."
        if command -v apt-get >/dev/null 2>&1; then
            sudo apt-get update
            sudo apt-get install -y build-essential cmake ninja-build libopencv-dev clang-format clang-tidy
        fi
        ;;
    *)
        echo "Unsupported OS: ${OS_TYPE}. Please refer to docs/architecture.md."
        ;;
esac

echo "==> Developer environment setup complete."
