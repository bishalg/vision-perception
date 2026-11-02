#!/usr/bin/env bash
# owner: WP-B.18 (DPL-10)
#
# Move the clinic server to another release:
#
#   sudo ./upgrade.sh 0.2.0            pull the images for 0.2.0 (ASD_IMAGE_PREFIX registry)
#   sudo ./upgrade.sh 0.2.0 --build    build them from this checkout instead
#
# Order: read README.md "Upgrading" for the version first (some releases need a step with
# the previous version), then this script: makes sure the images exist, runs backup.sh
# (stops if the backup fails: never upgrade without a fresh backup), switches ASD_VERSION
# in .env, runs `migrate`, restarts api / worker / console, waits until ready and prints
# the version the API reports. The same version again is safe: backup, a no-op migrate,
# restart.
set -euo pipefail
# shellcheck source=SCRIPTDIR/lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

usage() { echo "usage: sudo $0 <version> [--build]"; }
VERSION=""; BUILD=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --build) BUILD=1; shift ;;
    -h|--help) usage; exit 0 ;;
    -*) usage >&2; exit 2 ;;
    *) VERSION=$1; shift ;;
  esac
done
[[ -n "$VERSION" ]] || { usage >&2; exit 2; }
[[ "$VERSION" =~ ^[A-Za-z0-9._-]+$ ]] || die "'$VERSION' is not a version (letters, digits, . _ - only)."
require_root
require_env_file
require_docker

OLD=$(env_get ASD_VERSION)
say "Upgrade from $OLD to $VERSION"

# The images for the new version, without touching the running stack yet.
say "Getting the images for $VERSION"
if [[ -n "$BUILD" ]]; then
  # The backend first: the backup image is built FROM it (docker-image:// context).
  ASD_VERSION="$VERSION" dc build migrate || die "building the backend image for $VERSION failed."
  ASD_VERSION="$VERSION" dc --profile backup build console backup || die "building the images for $VERSION failed."
else
  ASD_VERSION="$VERSION" dc --profile backup pull --ignore-buildable api worker console backup 2>/dev/null || true
  prefix=$(env_get ASD_IMAGE_PREFIX)
  for img in asd-backend asd-console asd-backup; do
    docker image inspect "${prefix}$img:$VERSION" >/dev/null 2>&1 && continue
    if [[ -n "$prefix" ]]; then
      docker pull "${prefix}$img:$VERSION" >/dev/null || die "cannot download ${prefix}$img:$VERSION." \
        "Check the version number and the internet connection."
    else
      die "image $img:$VERSION is not on this server and no registry (ASD_IMAGE_PREFIX) is set." \
        "Build it from the source checkout:  sudo $0 $VERSION --build"
    fi
  done
fi
ok "images for $VERSION present"

say "Backup before upgrading"
"$ASD_DIR/backup.sh" || die "the backup failed, so the upgrade was NOT started (still on $OLD)."

say "Switching to $VERSION"
env_set ASD_VERSION "$VERSION"
if ! dc run --rm migrate >/dev/null; then
  env_set ASD_VERSION "$OLD"
  dc up -d >/dev/null || true
  die "the database migration for $VERSION failed; .env is back on $OLD." \
    "Read README.md 'Upgrading' for $VERSION. The backup made a minute ago is the newest in $(state_dir)/backups/."
fi
dc up -d --force-recreate api worker console >/dev/null
dc up -d >/dev/null
wait_ready 180 || die "the API is not ready after the upgrade." "Look at:  sudo $ASD_DIR/status.sh"
ok "upgraded: ASD_VERSION $VERSION, the API reports version $(backend_version)"
