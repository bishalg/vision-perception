# Applications

This directory contains end-user executable applications and integration demonstrators:

- **`ios_palm_app/`**: Palm Scanner **Xcode app** (app-local UI + ViewModels). Depends on shared packages only.
- **`android_palm_app/`**: Palm analysis Android app (shared `:overlay` library + app module).
- **`ios_vastu_app/`** / **`android_vastu_app/`**: Vastu / floor-plan apps (mandala reticle → quad ROI → `floor_plan` domain).
- **`mobile_overlay/`**: Shared iOS/Android camera overlay **packages/libraries** (guidance + Liquid Glass HUD + domain overlay plugins).
- **`desktop_pipeline_demo/`**: Interactive desktop demonstration over live webcam / video.
- **`cesium_viewer/`**: Phase 4 Vite + CesiumJS digital twin — CCTV pose markers, still panels, OSM globe.

Command-line evaluation and developer CLI tools live in [`tools/`](../tools/).
