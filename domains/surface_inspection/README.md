# Surface Inspection Domain Pack (`org.curv.domain.surface_inspection`)

Industrial quality-control interpretation of extracted ridges: classifies
sub-pixel curve segments as cracks / scratches and scores defect severity.

- **`SurfaceInspectionPack`** (`include/curv/domains/SurfaceInspectionPack.hpp`)
  — implements `IDomainPack`. Applies a `min_length_px` filter to reject noise,
  measures defect lengths in **unwarped source coordinates** (the C-ABI quad-ROI
  path re-inverts curve points through the homography inverse and recomputes
  true arc length before classification), and emits
  `domain_payload` fields such as `severe_cracks` and `max_defect_length`
  plus per-defect domain `Evidence`.

Typical entry point: `curv_pipeline_process_quad_roi` with domain
`"surface_inspection"`, or `curv_cli --domain surface_inspection --image ...`.
