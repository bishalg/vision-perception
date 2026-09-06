# Throughput Benchmarks

Tool: `benchmark` (or `curv_cli --benchmark`). Workload: synthetic horizontal
anti-aliased ridges (ellipse arcs), Steger config `sigma=1.5`,
`low=0.5, high=1.5`, `min_segment_length=5`. Sequential = `cv::setNumThreads(1)`,
parallel = `cv::setNumThreads(0)` (all cores via `cv::parallel_for_`).

## Apple MacBook (arm64, Apple clang 21, Release, 14 threads) — 2026-09-05

| Resolution  | Sequential (1 thread) | Multi-threaded | Speedup | FPS (parallel) |
|-------------|-----------------------|----------------|---------|----------------|
| 720p HD     | 7.05 ms               | 7.08 ms        | 0.99x   | 141            |
| 1080p FHD   | 16.40 ms              | 15.76 ms       | 1.04x   | 63             |

**Finding:** the `cv::parallel_for_` hessian path currently delivers no
meaningful speedup on this workload — per-tile work is too small relative to
pool scheduling overhead, or the dominant cost lies outside the parallelized
region. Follow-up: profile `StegerRidgeExtractor::extractRidgeGraph` stages and
coarsen the parallel tiles. Re-measure with `benchmark --iterations N` after
any change to the extraction pipeline.

Reproduce:

```bash
cmake --preset release && cmake --build --preset release -j
./build/tools/benchmark --iterations 7
```
