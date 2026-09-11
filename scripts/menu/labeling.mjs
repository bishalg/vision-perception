// 4. Labeling & training loop — gallery, compare, fix.
export const title = '[LABELING] Training data & labeling loop';
export const items = [
  {
    label: '[STUDIO] Launch CurvStudio (Roboflow-grade web annotation & test bench)',
    run: 'python3 scripts/studio_server.py --open',
  },
  {
    label: '[DOWNLOAD] Bootstrap ~100 CC-BY Roboflow palm labels (needs ROBOFLOW_API_KEY)',
    run: 'python3 scripts/download_palm_labels.py --limit 100',
  },
  {
    label: '[WEB] Palm line annotator (draw/tag Heart·Head·Life·Fate → USER_CORRECTED.json)',
    run: 'python3 tools/palm_annotator/server.py',
  },
  {
    label: '[DESKTOP GUI] Live Palm Crease Debugger (Interactive window + on-screen buttons)',
    run: './build/tools/palm_overlay_debug --image data/samples/mobile_captures/01_left_bishal.jpg',
  },
  {
    label: '[DESKTOP GUI] Debug custom image…',
    ask: 'Image path',
    run: (img) => `./build/tools/palm_overlay_debug --image ${img}`,
  },
  {
    label: '[WEB] Open Training & Admin Dashboard (Gallery · Truth mapping · Queue)',
    run: 'python3 scripts/admin_metrics.py && open output/admin_dashboard.html',
  },
  {
    label: '[WEB] Full Loop: Rebuild baselines + metrics + open Web Dashboard',
    run: 'python3 scripts/palm_crease_baseline_metrics.py --markdown && python3 scripts/admin_metrics.py && open output/admin_dashboard.html',
  },
  { label: '[CLI] Baseline metrics table (Terminal summary)', run: 'python3 scripts/palm_crease_baseline_metrics.py --markdown' },
  { label: '[CLI] Admin metrics history (Terminal summary)', run: 'python3 scripts/admin_metrics.py --markdown' },
  {
    label: '[NOTE] Defer ML until ~50–100 reviewed labels (no U-Net / heatmap export yet)',
    note: 'See data/palm_labels/README.md and docs/palm_robust_roadmap.md Week 3. Bad community labels would teach the wrong lines.',
  },
];
