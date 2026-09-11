// 4. Labeling & training loop — gallery, compare, fix.
export const title = 'Labeling & training loop';
export const items = [
  {
    label: 'Interactive overlay debugger (trackbars: o/l/r/d/h/s, q quits)',
    run: './build/tools/palm_overlay_debug --image data/samples/mobile_captures/01_left_bishal.jpg',
  },
  {
    label: 'Debug custom image…',
    ask: 'Image path',
    run: (img) => `./build/tools/palm_overlay_debug --image ${img}`,
  },
  { label: 'Baseline metrics table (all fixtures)', run: 'python3 scripts/palm_crease_baseline_metrics.py --markdown' },
  { label: 'Admin metrics: snapshot + history', run: 'python3 scripts/admin_metrics.py --markdown' },
  {
    label: 'Full loop: baselines + rebuild dashboard + open it',
    run: 'python3 scripts/palm_crease_baseline_metrics.py --markdown && python3 scripts/admin_metrics.py && open output/admin_dashboard.html',
  },
  { label: 'Open admin dashboard (matrix + trends + gallery)', run: 'open output/admin_dashboard.html' },
  { label: 'Open training gallery (same dashboard)', run: 'open output/admin_dashboard.html' },
];
