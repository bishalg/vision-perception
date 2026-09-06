# C-ABI (`libcurv_capi`) — The Stable Foreign-Function Boundary

The single stable contract every external consumer (JNI, Swift, CLI tools) is
built on. Header: [`include/curv/curv_capi.h`](include/curv/curv_capi.h) — the
ABI is kept backward-compatible; mobile consumers depend on it.

## Lifecycle

```c
CurvPipelineHandle p = curv_pipeline_create("palm");           // domain: "palm", "surface_inspection", or NULL
CurvFrameHandle f = curv_frame_create_from_buffer(             // wraps/copy-converts caller pixels
    buf, width, height, stride, CURV_PIXEL_FORMAT_BGRA);
int status = curv_pipeline_process_landmarks_roi(p, f, lms42, json_buf, sizeof(json_buf));
curv_frame_destroy(f);
curv_pipeline_destroy(p);
```

- **Frame ingestion**: grayscale / RGB / RGBA / BGR / BGRA, stride-aware. Validates
  dimensions (<= 16384 px per side), stride (>= width x bytes-per-pixel, non-negative),
  and format; returns NULL on any violation (logged via spdlog).
- **Processing**: full frame, quad ROI (`process_quad_roi`, 8 floats TL/TR/BR/BL),
  or landmark ROI (`process_landmarks_roi`, 42 floats = 21 normalized points) with
  canonical palm warp + sub-pixel coordinate inversion back to source space.
- **Results**: status codes (`SUCCESS`, `INVALID_ARGUMENT`, `QUALITY_GATE_REJECTED`,
  `INTERNAL_ERROR`, `BUFFER_TOO_SMALL`) plus a size-query pattern on
  `curv_pipeline_get_last_json` (pass NULL/0 to fetch the required buffer size).
- **Error safety**: no exception ever crosses the boundary; all failure paths are
  caught, logged, and mapped to status codes.

Built as both shared (`curv_capi`) and static (`curv_capi_static`) targets.
