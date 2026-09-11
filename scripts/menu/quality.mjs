// 6. Quality gates & benchmarks.
export const title = 'Quality gates & benchmarks';
export const items = [
  { label: 'Format check (CI gate)', run: './scripts/format.sh --check' },
  { label: 'Format write (in place)', run: './scripts/format.sh' },
  { label: 'Lint (clang-tidy)', run: './scripts/lint.sh' },
  { label: 'Coverage gate (Linux CI; known-broken on macOS Xcode 27)', run: './scripts/coverage.sh' },
  { label: 'ASan + UBSan build & test', run: 'cmake --preset asan && cmake --build --preset asan -j && ctest --test-dir build-asan' },
  { label: 'Full benchmark (720p/1080p/4K)', run: './build/tools/benchmark' },
];
