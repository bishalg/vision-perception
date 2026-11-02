#!/usr/bin/env bash
# owner: WP-B.18 (DPL-1, DPL-6, DPL-8)
#
# One screen that says whether the clinic server is healthy:
#
#   sudo ./status.sh
#
# Shows the containers, the version the API reports, readiness through Caddy, the newest
# backup and its age (a failed last backup is an error), and the last restore drill (FAIL,
# or older than 45 days, is a warning). Exit 0 when nothing needs attention, 1 otherwise.
set -euo pipefail
# shellcheck source=SCRIPTDIR/lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"
require_root
require_env_file
require_docker

STATE=$(state_dir)
DOMAIN=$(env_get DOMAIN)
attention=0
age_days() { echo $(( ($(date +%s) - $1) / 86400 )); }
mtime() { stat -c %Y "$1" 2>/dev/null || stat -f %m "$1"; }

say "Clinic server status ($DOMAIN, TLS mode $(env_get TLS_MODE), ASD_VERSION $(env_get ASD_VERSION))"
dc ps --format 'table {{.Service}}\t{{.State}}\t{{.Status}}\t{{.Ports}}'

if wait_ready 3; then
  ok "https://api.$DOMAIN/health/ready answers 200 (API version $(backend_version))"
else
  warn "https://api.$DOMAIN/health/ready is NOT ready. Start the stack: sudo $ASD_DIR/install.sh"
  attention=1
fi

newest=$(find "$STATE/backups" -maxdepth 1 -name 'asd-*.tar.age' 2>/dev/null | sort | tail -n 1)
if [[ -f "$STATE/backups/LAST_FAILED" ]]; then
  warn "the last backup FAILED: $(cat "$STATE/backups/LAST_FAILED"). Run: sudo $ASD_DIR/backup.sh"
  attention=1
fi
if [[ -n "$newest" ]]; then
  d=$(age_days "$(mtime "$newest")")
  if ((d > 1)); then
    warn "newest backup $(basename "$newest") is $d days old"
    attention=1
  else
    ok "newest backup $(basename "$newest") ($d days old, $(find "$STATE/backups" -maxdepth 1 -name 'asd-*.tar.age' | wc -l | tr -d ' ') kept)"
  fi
else
  warn "no backup yet. Run: sudo $ASD_DIR/backup.sh"
  attention=1
fi

last=$(find "$STATE/drills" -maxdepth 1 \( -name '*.PASS' -o -name '*.FAIL' \) 2>/dev/null | sort | tail -n 1)
if [[ -z "$last" ]]; then
  warn "no restore drill has run yet. Run: sudo $ASD_DIR/restore-drill.sh"
  attention=1
else
  d=$(age_days "$(mtime "$last")")
  case "$last" in
    *.FAIL) warn "the last restore drill FAILED ($(basename "$last"), $d days ago): $(cut -d' ' -f2- "$last")"; attention=1 ;;
    *) if ((d > 45)); then
         warn "the last restore drill passed, but $d days ago (more than 45)"; attention=1
       else
         ok "last restore drill PASS ($(basename "$last" .PASS), $d days ago)"
       fi ;;
  esac
fi

df -h "$STATE" | tail -n 1 | awk '{print "    disk  " $4 " free of " $2 " on " $NF}'
exit $attention
