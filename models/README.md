# Machine Learning Models Directory

This directory manages neural network models, weight manifests, and licensing records for models consumed by adapters and domain packs.

## Governance Rules
1. **No Large Binary Blobs in Git**: Model weight files (`.onnx`, `.tflite`) exceeding 10MB must not be committed to Git history. They must be downloaded via `scripts/download_models.sh`.
2. **Mandatory Manifests**: Every model must have a corresponding manifest in `models/manifests/<model_name>.json` defining:
   - Model name, architecture, and version.
   - Author, upstream source URL, and commit/release tag.
   - License classification (must comply with `docs/license_policy.md`).
   - SHA-256 cryptographic checksum of the weight binary.
   - Input and output tensor signatures.
3. **License Copies**: Full license texts for all consumed models must be stored in `models/licenses/`.
