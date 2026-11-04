#!/usr/bin/env bash
# Live researcher-toolkit e2e (phase 8, research.live.ts): the console in live mode against asd-backend serve + worker
# on a scratch database, seeded by seed-research.sh. Same environment as run-record-live.sh (PG_URL required).
#
#   PG_URL=postgres://postgres@127.0.0.1:5432 apps/curv_console/e2e/live/run-research-live.sh
SUITE=research exec "$(dirname "${BASH_SOURCE[0]}")/run-record-live.sh" "$@"
