# Perception Test Data Directory

This directory stores datasets, synthetic curve fixtures, and test annotations used for unit tests, regression tests, and precision validation:

- **`fixtures/`**: Static real-world reference datasets (CCTV, satellite crops, flood corridors) supporting the [Gradual 3-Stage Development Strategy](fixtures/README.md) (Static Images $\to$ Recorded Video $\to$ Live Streams).
- **`samples/`**: Small, representative real-world images for sanity checks and integration tests.
- **`synthetic/`**: Mathematically generated curves (straight lines, parabolas, sinusoids, concentric rings) with exact known analytical sub-pixel ground truth.
- **`annotations/`**: Ground-truth JSON annotations formatted according to `docs/evidence_schema.md`.
- **`flood_nepal_2026/`**: Historical Sentinel-2 L2A optical disaster fixtures for the Lende Khola / Bhote Koshi event.

## Data Governance
- Only small fixtures (<5MB total) are committed directly to version control.
- Heavy benchmark datasets must be fetched via external scripts or streamed.

