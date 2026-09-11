// 3. Core C++ engine — build, test, demos.
export const title = 'Core C++ engine';
export const items = [
  { label: 'Build everything (Release)', run: 'cmake --preset release && cmake --build --preset release -j' },
  { label: 'Run full test suite (ctest)', run: 'ctest --test-dir build --output-on-failure' },
  {
    label: 'curv_cli on sample palm (palm domain)',
    run: './build/tools/curv_cli --image data/samples/mobile_captures/01_left_bishal.jpg --domain palm --dark --out output/cli_sample.png --json output/cli_sample.json',
  },
  { label: 'curv_cli sanity check', run: './build/tools/curv_cli' },
  { label: 'Quick benchmark (20 iters)', run: './build/tools/benchmark --iterations 20' },
  {
    label: 'Synthetic curve demo (synth_generator)',
    run: './build/tools/synth_generator --help || ./build/tools/synth_generator',
  },
];
