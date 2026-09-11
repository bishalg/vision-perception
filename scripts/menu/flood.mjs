// 2. Flood & geospatial — pipeline, delta, twin.
export const title = 'Flood & river morphology';
export const items = [
  { label: 'Run full flood pipeline (PRE + POST Steger)', run: './scripts/run_flood_pipeline.sh' },
  { label: 'Compute flood delta report', run: 'python3 scripts/compute_flood_delta.py' },
  { label: 'Check PRE/POST registration', run: 'python3 scripts/check_registration.py' },
  { label: 'Prepare flood data (ingest pair)', run: 'python3 scripts/prepare_flood_data.py' },
  { label: 'Sigma sweep on POST image', run: 'python3 scripts/sweep_sigma.py' },
  { label: 'F1 sweep (weakly-supervised)', run: 'python3 scripts/sweep_f1.py' },
  { label: 'Open Cesium 3D twin (vite dev)', run: 'pnpm --dir apps/cesium_viewer dev' },
  { label: 'Build Cesium twin (static dist)', run: 'pnpm --dir apps/cesium_viewer build' },
];
