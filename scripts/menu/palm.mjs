import { execSync } from 'node:child_process';

function getConnectedDevices() {
  try {
    const raw = execSync('xcrun devicectl list devices --json-output /dev/stdout 2>/dev/null || true', {
      encoding: 'utf8',
    });
    const data = JSON.parse(raw);
    const devs = data?.result?.devices || [];
    return devs.map((d) => {
      const hw = d.hardwareProperties || {};
      const dev_p = d.deviceProperties || {};
      const conn = d.connectionProperties || {};
      const props = d.properties || {};
      const name = dev_p.name || hw.marketingName || 'Unknown Device';
      const model = hw.marketingName || dev_p.marketingName || '';
      const udid = hw.udid || d.identifier;
      const state = conn.tunnelState || props.connection?.state || 'unknown';
      return { name, model, udid, state };
    });
  } catch {
    return [];
  }
}

function getDeviceMenuItems() {
  const devs = getConnectedDevices();
  if (devs.length === 0) {
    return [
      {
        label: 'Default iPhone (Bishal’s iPhone 17 Pro)',
        run: './scripts/run_ios_device.sh 00008150-001409290AF0401C',
      },
    ];
  }
  return devs.map((d) => {
    const statusNote = d.state === 'unavailable' ? ' [unavailable]' : ' [ready]';
    return {
      label: `${d.name} (${d.model || d.udid.slice(0, 8)})${statusNote}`,
      run: `./scripts/run_ios_device.sh ${d.udid}`,
    };
  });
}

// 1. Palm reading apps — iOS / Android / desktop.
export const title = '[APPS] Palm reading (iOS · Android · Desktop)';
export const items = [
  { label: '[MOBILE] Run iOS App (Simulator build + launch)', run: './scripts/run_ios_sim.sh' },
  {
    label: '[MOBILE] Run on Connected iPhone / iPad…',
    menu: getDeviceMenuItems,
  },
  { label: '[MOBILE] Open iOS Project in Xcode', run: 'open apps/ios_palm_app/PalmScanner.xcodeproj' },
  { label: '[MOBILE] Build Android AAR (Native .so)', run: './scripts/build_android_aar.sh' },
  { label: '[MOBILE] Open Android App in Android Studio', run: 'open apps/android_palm_app' },
  {
    label: '[DESKTOP GUI] Live Palm Crease Debugger (Interactive window + sliders)',
    run: './build/tools/palm_overlay_debug --image data/samples/mobile_captures/01_left_bishal.jpg',
  },
  {
    label: '[CLI] Run Hand Inspector on Left Hand (Writes output PNG + JSON)',
    run: './build/tools/hand_inspector_cli --image data/samples/mobile_captures/01_left_bishal.jpg --auto-landmarks --left-hand --out-json output/palm_01_left.json --out-full-overlay output/palm_01_left_full.png',
  },
  {
    label: '[CLI] Run Hand Inspector on Right Hand (Writes output PNG + JSON)',
    run: './build/tools/hand_inspector_cli --image data/samples/mobile_captures/02_right_bishal.jpg --auto-landmarks --out-json output/palm_02_right.json --out-full-overlay output/palm_02_right_full.png',
  },
  {
    label: '[CLI] Run Hand Inspector on Custom Image…',
    ask: 'Image path',
    run: (img) =>
      `./build/tools/hand_inspector_cli --image ${img} --auto-landmarks --out-json output/palm_custom.json --out-full-overlay output/palm_custom_full.png`,
  },
];
