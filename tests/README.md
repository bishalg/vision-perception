# Test Suite

GoogleTest (1.14) unit + regression tests, executed via CTest. CI runs this on
Ubuntu and macOS with warnings-as-errors, under ASan+UBSan, and enforces a
100% line+branch coverage gate (`scripts/coverage.sh`).

```bash
ctest --test-dir build                 # run everything
ctest --test-dir build -R steger -V    # run one suite verbosely
./scripts/coverage.sh                  # coverage pipeline + 100% gate
```

## Suites

| Target | Covers |
|---|---|
| `test_geometry_types` | `Point2D` / `CurveSegment` / `Junction` / `RidgeGraph` value semantics |
| `test_quality_analyzer` | Laplacian focus/blur quality gate, thresholds, rejections |
| `test_steger_extractor` | Ridge extraction on synthetic curves, config guards |
| `test_parameter_resolver` | Dynamic Steger parameter scaling from ROI diagonal |
| `test_roi_warper` | Quad perspective warp, sub-pixel coordinate inversion |
| `test_zone_intersector` | Polygon zone hit-testing |
| `test_palm_domain` | Palm crease classification, zone tagging |
| `test_surface_inspection` | Crack/scratch defect scoring |
| `test_overlay_renderer` | Sub-pixel overlay rendering incl. small-image safety |
| `test_vlm_formatter` | Grounded VLM prompt construction |
| `test_capi` / `test_regression_audit` | C-ABI contract, buffer safety, handedness, FFI stride/dimension validation |
| `test_demo_lib`, CLI suites | `tools/lib/` testable CLI libraries |
| `evaluation_rmse_gate` (CTest script) | End-to-end accuracy gate: synth_generator → evaluate, requires RMSE < 0.1 px on synthetic line/parabola/spiral |

Tests build synthetic `cv::Mat` fixtures inline; static sample data lives in `data/samples/`.
Exemptions to the coverage gate are whitelist-only with written justifications
(`scripts/coverage_exemptions.txt`) and are reserved for provably unreachable defensive code.
