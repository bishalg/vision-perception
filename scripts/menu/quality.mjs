// 6. Quality gates & benchmarks.
export const title = '[QUALITY] Quality gates & benchmarks';
export const items = [
  { label: '[GATE] Format check (clang-format CI gate)', run: './scripts/format.sh --check' },
  { label: '[TOOL] Format write (clang-format in place)', run: './scripts/format.sh' },
  { label: '[GATE] Run linter (clang-tidy)', run: './scripts/lint.sh' },
  { label: '[GATE] Coverage gate (Linux CI; known-broken on macOS Xcode 27)', run: './scripts/coverage.sh' },
  { label: '[GATE] Build & test with ASan + UBSan sanitizers', run: 'cmake --preset asan && cmake --build --preset asan -j && ctest --test-dir build-asan' },
  { label: '[BENCHMARK] Full resolution benchmark (720p · 1080p · 4K)', run: './build/tools/benchmark' },
];
