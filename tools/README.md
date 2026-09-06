# Developer & Research Tools

Utilities, CLI executables, and benchmarking tools for pipeline execution, sub-pixel accuracy evaluation, and throughput profiling.

All executable CLIs are built with thin mains backed by the testable static support library `curv_tools_lib` (`tools/lib/curv/tools/`).

## Executables & Libraries

- **`curv_cli`** (`curv_cli.cpp` / `cli_lib.cpp`): Primary CLI runner for batch and single-image perception processing. Supports quad ROI, domain pack selection (`palm`, `surface_inspection`), sub-pixel overlay rendering, and JSON evidence export.
- **`hand_inspector_cli`** (`hand_inspector_cli.cpp` / `hand_inspector_lib.cpp`): Palm-specific inspection CLI. Integrates ONNX hand landmark detection, canonical 512x512 ROI warping, crease extraction, and grounded Gemini/Gemma VLM prompt generation.
- **`synth_generator`** (`synth_generator.cpp` / `synth_lib.cpp`): Synthetic ground-truth curve generator. Produces anti-aliased images and analytical truth polylines (0.25 px sampling) for lines, parabolas, and spirals with configurable Gaussian noise.
- **`evaluate`** (`evaluate.cpp` / `evaluate_lib.cpp`): Quantitative accuracy evaluator. Matches extracted ridges to mathematical ground truth, computes point-to-polyline RMSE and error metrics, and enforces the sub-0.1 pixel accuracy gate.
- **`benchmark`** (`benchmark.cpp` / `benchmark_lib.cpp`): Throughput and latency profiling harness measuring Gaussian filtering, Hessian eigendecomposition, Steger extraction, and quality gating at 720p, 1080p, and 4K resolutions.

## Automated Accuracy Gate

- **`eval_gate.cmake`**: CTest script running `synth_generator` -> `curv_cli` -> `evaluate`. Enforces RMSE < 0.1 px across synthetic curves as part of standard test execution (`evaluation_rmse_gate`).

