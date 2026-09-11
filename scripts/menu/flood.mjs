// 2. Flood & geospatial — pipeline, delta, twin.
export const title = '[GEOSPATIAL] Flood & river morphology';
export const items = [
  { label: '[STUDIO] Annotate & Verify Flood Lines (CurvStudio)', run: 'python3 scripts/studio_server.py --open' },
  { label: '[CLI] Run full flood pipeline (PRE + POST Steger)', run: './scripts/run_flood_pipeline.sh' },
  { label: '[CLI] Compute flood delta report', run: 'python3 scripts/compute_flood_delta.py' },
  { label: '[CLI] Check PRE/POST registration', run: 'python3 scripts/check_registration.py' },
  { label: '[CLI] Prepare flood data (Ingest pair)', run: 'python3 scripts/prepare_flood_data.py' },
  { label: '[CLI] Sigma sweep on POST image', run: 'python3 scripts/sweep_sigma.py' },
  { label: '[CLI] F1 sweep (Weakly-supervised)', run: 'python3 scripts/sweep_f1.py' },
  { label: '[WEB] Open Cesium 3D Digital Twin (Vite dev server)', run: 'pnpm --dir apps/cesium_viewer dev' },
  { label: '[BUILD] Build Cesium 3D Twin (Static bundle)', run: 'pnpm --dir apps/cesium_viewer build' },
];
