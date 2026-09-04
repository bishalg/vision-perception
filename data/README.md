# Perception Test Data Directory

This directory stores datasets, synthetic curve fixtures, and test annotations used for unit tests, regression tests, and precision validation:

- **`samples/`**: Small, representative real-world images for sanity checks and integration tests.
- **`synthetic/`**: Mathematically generated curves (straight lines, parabolas, sinusoids, concentric rings) with exact known analytical sub-pixel ground truth.
- **`annotations/`**: Ground-truth JSON annotations formatted according to `docs/evidence_schema.md`.

## Data Governance
- Only small fixtures (<5MB total) are committed directly to version control.
- Heavy benchmark datasets must be fetched via external scripts.
