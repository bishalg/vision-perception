# Throughput Benchmarks

Tool: `benchmark` (or `curv_cli --benchmark`). Workload: synthetic horizontal
anti-aliased ridges (ellipse arcs), Steger config `sigma=1.5`,
`low=0.5, high=1.5`, `min_segment_length=5`. Sequential = `cv::setNumThreads(1)`,
parallel = `cv::setNumThreads(0)` (all cores via `cv::parallel_for_`).

## Apple MacBook (arm64, Apple clang 21, Release, 14 threads) — 2026-09-06

| Resolution  | Sequential (1 thread) | Multi-threaded | Speedup | FPS (parallel) |
|-------------|-----------------------|----------------|---------|----------------|
| 720p HD     | 6.45 ms               | 6.45 ms        | 1.00x   | 155            |
| 1080p FHD   | 15.22 ms              | 14.60 ms       | 1.04x   | 69             |
| 4K UHD      | 61.61 ms              | 61.47 ms       | 1.00x   | 16.3           |

**Finding:** the `cv::parallel_for_` hessian path currently delivers no
meaningful speedup on this workload — per-tile work is too small relative to
pool scheduling overhead, or the dominant cost lies outside the parallelized
region. Follow-up: profile `StegerRidgeExtractor::extractRidgeGraph` stages and
coarsen the parallel tiles. Re-measure with `benchmark --iterations N` after
any change to the extraction pipeline.

Latency budget guidance: at 1080p the engine sustains real-time (>30 FPS) on a
single thread; at 4K plan for ~16 FPS per frame budget on this class of hardware
or move extraction into the quality-gated ROI path (512x512 canonical patches)
where per-frame cost drops by an order of magnitude.

Reproduce:

```bash
cmake --preset release && cmake --build --preset release -j
./build/tools/benchmark --iterations 7
```
