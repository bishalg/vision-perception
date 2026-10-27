#!/usr/bin/env bash
# SIM-6 / ADR-0004: a Release binary must not contain Debug-only strings.
# Usage: scripts/check_release_symbols.sh <path to Release AsdGazeCollector binary>
set -euo pipefail
bin="${1:?usage: $0 <release binary>}"
[ -f "$bin" ] || { echo "no such file: $bin" >&2; exit 2; }
fail=0
for needle in "Switch to Simulation" "Elevated Risk" "BlendShapeML" "ASD_DEMO_TOOLS" "DemoToolsView"; do
  if strings -a "$bin" | grep -qF "$needle"; then
    echo "FORBIDDEN string in Release binary: $needle" >&2
    fail=1
  fi
done
[ "$fail" -eq 0 ] && echo "release symbols ok"
exit "$fail"
