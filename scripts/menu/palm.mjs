// 1. Palm reading apps — iOS / Android / desktop.
export const title = 'Palm reading apps';
export const items = [
  { label: 'Run iOS app on simulator (build + install + launch)', run: './scripts/run_ios_sim.sh' },
  { label: 'Build + run on connected iPhone', run: './scripts/run_ios_device.sh' },
  { label: 'Open iOS project in Xcode', run: 'open apps/ios_palm_app/PalmScanner.xcodeproj' },
  { label: 'Build Android AAR (native .so)', run: './scripts/build_android_aar.sh' },
  { label: 'Open Android app in Android Studio', run: 'open apps/android_palm_app' },
  {
    label: 'Inspect 01_left on desktop (hand_inspector_cli)',
    run: './build/tools/hand_inspector_cli --image data/samples/mobile_captures/01_left_bishal.jpg --auto-landmarks --left-hand --out-json output/palm_01_left.json --out-full-overlay output/palm_01_left_full.png',
  },
  {
    label: 'Inspect 02_right on desktop (hand_inspector_cli)',
    run: './build/tools/hand_inspector_cli --image data/samples/mobile_captures/02_right_bishal.jpg --auto-landmarks --out-json output/palm_02_right.json --out-full-overlay output/palm_02_right_full.png',
  },
  {
    label: 'Inspect custom image…',
    ask: 'Image path',
    run: (img) =>
      `./build/tools/hand_inspector_cli --image ${img} --auto-landmarks --out-json output/palm_custom.json --out-full-overlay output/palm_custom_full.png`,
  },
];
