#!/usr/bin/env bash
# SIM-6 / ADR-0004: a Release binary must not contain Debug-only strings.
# WP-G.8 / BEN-2 / ADR-0009 D10: neither Release nor DemoRelease may contain the bench
# capture build (ASD_BENCH_CAPTURE is Debug-only).
#
# Store build (store-collection-build.md S-1, S-3): Release defines ASD_STORE_BUILD, so the
# result screen's report sharing, FHIR export and full-screen replay are not compiled into it.
#
# Usage: scripts/check_release_symbols.sh <path to AsdGazeCollector binary> [Release|DemoRelease]
#   Release (default): demo tools, simulation, the bench and the store-hidden result actions are forbidden.
#   DemoRelease:       demo tools are expected; only the bench is forbidden.
set -euo pipefail
bin="${1:?usage: $0 <binary> [Release|DemoRelease]}"
config="${2:-Release}"
[ -f "$bin" ] || { echo "no such file: $bin" >&2; exit 2; }
case "$config" in
  Release)
    needles=("Switch to Simulation" "Elevated Risk" "BlendShapeML" "ASD_DEMO_TOOLS" "DemoToolsView"
             "Share Report for Your Pediatrician" "Return to Dashboard" "Open Full-Screen Replay") ;;
  DemoRelease)
    needles=() ;;
  *) echo "unknown configuration: $config (expected Release or DemoRelease)" >&2; exit 2 ;;
esac
# Bench capture (WP-G.8): the flag literal, the app types and the kit's tap. Valid for both.
needles+=("ASD_BENCH_CAPTURE" "BenchCaptureView" "BenchLogWriter" "BenchRunConfig" "BenchRawSample" "setBenchTap")
# Store screenshot fixture (WP-S.7): compiled into the StoreSim configuration only. Valid for both.
needles+=("ASDStoreScreenshotFixture" "ASDStoreScreenshotScreen")
fail=0
syms="$(strings -a "$bin")"
for needle in "${needles[@]}"; do
  if grep -qF "$needle" <<<"$syms"; then
    echo "FORBIDDEN string in $config binary: $needle" >&2
    fail=1
  fi
done
[ "$fail" -eq 0 ] && echo "release symbols ok ($config)"
exit "$fail"
