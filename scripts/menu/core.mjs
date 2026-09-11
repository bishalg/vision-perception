// 3. Core C++ engine — build, test, demos.
export const title = '[ENGINE] Core C++ engine';
export const items = [
  { label: '[BUILD] Build everything (Release)', run: 'cmake --preset release && cmake --build --preset release -j' },
  { label: '[TEST] Run full test suite (ctest)', run: 'ctest --test-dir build --output-on-failure' },
  {
    label: '[CLI] Run Curv CLI on sample palm',
    run: './build/tools/curv_cli --image data/samples/mobile_captures/01_left_bishal.jpg --domain palm --dark --out output/cli_sample.png --json output/cli_sample.json',
  },
  { label: '[CLI] Run Curv CLI sanity check', run: './build/tools/curv_cli' },
  { label: '[BENCHMARK] Run quick benchmark (20 iterations)', run: './build/tools/benchmark --iterations 20' },
  {
    label: '[CLI] Run synthetic curve demo (synth_generator)',
    run: './build/tools/synth_generator --help || ./build/tools/synth_generator',
  },
];
