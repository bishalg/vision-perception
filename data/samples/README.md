# Data Samples Policy & Guidelines

This directory contains test fixtures and sample data for algorithm validation and visual perception testing.

## 1. File Size Limit Policy
- **Maximum File Size:** No tracked binary or sample file in the repository may exceed **1 MB**.
- Any large dataset or high-resolution capture must be stored externally (e.g., S3 / release assets) and downloaded via scripts.

## 2. Privacy & Personal Data Policy
- **No Personal Identifiable Information (PII):** Do not commit personal photographs, named captures, or biometric data belonging to real, identifiable individuals without explicit written consent and formal anonymization.
- **Synthetics Preferred:** New test samples must be synthetic, fully anonymized, or public-domain reference fixtures.
- **Named captures (`*bishal*`)**: Untracked and excluded from git tracking via `.gitignore`.
