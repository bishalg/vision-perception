# OpenCV Overlay Renderer

`OverlayRenderer` (`include/curv/adapters/OverlayRenderer.hpp`) draws the
perception result back onto the source image:

- Anti-aliased sub-pixel polylines (configurable 2^n supersampling, default 16x),
- Optional normal vectors ("whiskers") and width ribbons,
- Quality HUD (blur/brightness/contrast + usability),
- `exportToSvg` for vector export of extracted curves.

Handles grayscale / BGR / BGRA inputs and tiny images safely (covered by
regression tests). Returns a BGR canvas ready for `cv::imwrite` or display.
