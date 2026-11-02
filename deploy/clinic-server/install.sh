#!/usr/bin/env bash
# owner: WP-B.18 (DPL-2)
#
# One-command install of the clinic server (Debian / Ubuntu with Docker Engine 25+):
#
#   sudo ./install.sh
#
# Asks four questions (web address, certificate mode, administrator e-mail, clinic name),
# creates every secret, starts the stack, creates the clinic and its owner account,
# installs the nightly backup and monthly restore-drill timers and prints the KEY CARD
# once. Running it again on an installed server changes nothing and prints the status.
#
# Without questions (automation, CI):
#   sudo ./install.sh --non-interactive --domain clinic.example.org --tls letsencrypt \
#        --admin-email it@example.org --org-name "Example Clinic"
# Options:
#   --tls letsencrypt|internal|tunnel   certificate mode (README "TLS modes")
#   --tunnel-token TOKEN                Cloudflare Tunnel token (tls tunnel only)
#   --allow-unencrypted-disk            install although the data disk is not encrypted
#   --version V                         release to run (default: the source checkout's)
#   --build                             build the images from this checkout (default
#                                       when ASD_IMAGE_PREFIX is empty)
# Development / test only (environment): ASD_STATE_DIR (default /var/lib/asd), HTTP_PORT,
# HTTPS_PORT, ASD_ENV_FILE, ASD_PROJECT, ASD_DEV_NON_ROOT=1.
set -euo pipefail
umask 077
# shellcheck source=SCRIPTDIR/lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

NONINTERACTIVE=""; DOMAIN=""; TLS_MODE=""; ADMIN_EMAIL=""; ORG_NAME=""; TUNNEL_TOKEN=""
ALLOW_UNENCRYPTED=""; VERSION=""; BUILD=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --non-interactive) NONINTERACTIVE=1; shift ;;
    --domain) DOMAIN=${2:-}; shift 2 ;;
    --tls) TLS_MODE=${2:-}; shift 2 ;;
    --admin-email) ADMIN_EMAIL=${2:-}; shift 2 ;;
    --org-name) ORG_NAME=${2:-}; shift 2 ;;
    --tunnel-token) TUNNEL_TOKEN=${2:-}; shift 2 ;;
    --allow-unencrypted-disk) ALLOW_UNENCRYPTED=1; shift ;;
    --version) VERSION=${2:-}; shift 2 ;;
    --build) BUILD=1; shift ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option: $1" "See:  $0 --help" ;;
  esac
done

require_root
require_docker
command -v curl >/dev/null || die "curl is missing." "Install it with:  sudo apt-get install curl"

STATE="${ASD_STATE_DIR:-$(env_get ASD_STATE_DIR)}"
STATE="${STATE:-/var/lib/asd}"
PENDING="$STATE/install.pending"

# ------------------------------------------------------------------ helpers
rand_hex() { od -An -tx1 -N"$1" /dev/urandom | tr -d ' \n'; }
rand_b64() { head -c "$1" /dev/urandom | base64 | tr -d '\n'; }
# Readable owner password: 4 groups of 5 from an alphabet without look-alikes.
rand_password() {
  local s
  s=$(LC_ALL=C tr -dc 'abcdefghjkmnpqrstuvwxyz23456789' </dev/urandom | head -c 20)
  printf '%s-%s-%s-%s' "${s:0:5}" "${s:5:5}" "${s:10:5}" "${s:15:5}"
}
pending_get() { env_get "$1" "$PENDING"; }

valid_domain() {
  [[ "$1" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$ ]] || return 1
  [[ ! "$1" =~ ^[0-9.]+$ ]]   # iOS refuses IP addresses (url_rules): a name is required
}
valid_email() { [[ "$1" =~ ^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$ ]]; }

ask() { # ask VAR "question" validator "error text"
  local var=$1 q=$2 check=$3 err=$4 answer
  while true; do
    read -r -p "    $q " answer
    if "$check" "$answer"; then printf -v "$var" '%s' "$answer"; return; fi
    printf '    %s\n' "$err"
  done
}
valid_tls() { [[ "$1" == letsencrypt || "$1" == internal || "$1" == tunnel ]]; }
non_empty() { [[ -n "$1" && ${#1} -le 200 ]]; }

# DPL-2: clinical data must sit on an encrypted disk (LUKS), checked with lsblk.
check_disk() {
  if ! command -v lsblk >/dev/null || ! command -v findmnt >/dev/null; then
    warn "cannot check disk encryption on this system (no lsblk/findmnt): SKIPPED"
    return 0
  fi
  local dir src docker_root bad=()
  docker_root=$(docker info --format '{{.DockerRootDir}}' 2>/dev/null || echo /var/lib/docker)
  mkdir -p "$STATE"
  for dir in "$STATE" "$docker_root"; do
    src=$(findmnt -n -o SOURCE --target "$dir" 2>/dev/null | head -n 1)
    src=${src%%\[*}
    if lsblk -s -n -o TYPE "$src" 2>/dev/null | grep -qw crypt; then continue; fi
    if grep -qE "^/dev/mapper/[^ ]+ $(findmnt -n -o TARGET --target "$dir") " /proc/mounts 2>/dev/null \
      && lsblk -n -o TYPE,FSTYPE 2>/dev/null | grep -qw crypt; then continue; fi
    bad+=("$dir (on $src)")
  done
  [[ ${#bad[@]} -eq 0 ]] && { ok "data disk is encrypted"; return 0; }
  if [[ -n "$ALLOW_UNENCRYPTED" ]]; then
    warn "the data disk is NOT encrypted (${bad[*]}); continuing because of --allow-unencrypted-disk"
    return 0
  fi
  die "the data disk is NOT encrypted: ${bad[*]}." \
    "Patient data must be on an encrypted disk. Reinstall Ubuntu with 'Encrypt the new" \
    "installation' (LUKS), or put $STATE and Docker's data on an encrypted volume." \
    "For a throwaway test server only: add --allow-unencrypted-disk."
}

ensure_images() {
  local prefix version
  prefix=$(env_get ASD_IMAGE_PREFIX); version=$(env_get ASD_VERSION)
  if [[ -z "$BUILD" ]]; then
    local missing=0 img
    for img in asd-backend asd-console asd-backup; do
      docker image inspect "${prefix}$img:$version" >/dev/null 2>&1 || missing=1
    done
    [[ $missing -eq 0 ]] && { ok "images for $version present"; return 0; }
    if [[ -n "$prefix" ]]; then
      say "Downloading the images for $version"
      dc --profile backup pull api console backup || die "cannot download the images for $version." \
        "Check the internet connection and ASD_IMAGE_PREFIX in $ASD_ENV_FILE."
      return 0
    fi
  fi
  [[ -f "$ASD_DIR/../../rust/Dockerfile" ]] || die "no images and no source checkout to build them from." \
    "Set ASD_IMAGE_PREFIX in $ASD_ENV_FILE to the image registry, then run this again."
  say "Building the images for $version (the first build takes 10-30 minutes)"
  # The backend first: the backup image is built FROM it (docker-image:// context).
  dc build migrate || die "building the backend image failed (see above)."
  dc --profile backup build console backup || die "building the images failed (see above)."
}

# ------------------------------------------------------------------ already installed?
if [[ -f "$ASD_ENV_FILE" && ! -f "$PENDING" ]]; then
  say "This server is already installed ($ASD_ENV_FILE exists). Nothing was changed."
  dc up -d >/dev/null 2>&1 || warn "could not start the stack; see the status below"
  "$ASD_DIR/status.sh" || true
  exit 0
fi

# ------------------------------------------------------------------ questions + secrets
if [[ ! -f "$ASD_ENV_FILE" ]]; then
  say "Clinic server installation"
  if [[ -z "$NONINTERACTIVE" ]]; then
    [[ -t 0 ]] || die "no terminal to ask questions on." "Run it in a terminal, or use --non-interactive with the options (see --help)."
    echo "    Four questions. Press Ctrl-C at any time to stop; nothing is changed until the end."
    [[ -n "$DOMAIN" ]] || ask DOMAIN "1/4 Web address of this server (for example clinic.example.org):" \
      valid_domain "Use a name like clinic.example.org (lower case, no https://, not an IP address)."
    [[ -n "$TLS_MODE" ]] || ask TLS_MODE "2/4 Certificates: letsencrypt (reachable from the internet), internal (clinic network only) or tunnel (Cloudflare)?" \
      valid_tls "Type one of: letsencrypt, internal, tunnel"
    [[ -n "$ADMIN_EMAIL" ]] || ask ADMIN_EMAIL "3/4 E-mail of the administrator (becomes the owner sign-in):" \
      valid_email "That does not look like an e-mail address."
    [[ -n "$ORG_NAME" ]] || ask ORG_NAME "4/4 Name of the clinic:" non_empty "Type a name (up to 200 characters)."
    if [[ "$TLS_MODE" == tunnel && -z "$TUNNEL_TOKEN" ]]; then
      read -r -s -p "    Cloudflare Tunnel token (from the Cloudflare dashboard, hidden): " TUNNEL_TOKEN; echo
    fi
  fi
  missing=()
  valid_domain "$DOMAIN" || missing+=("--domain (a name like clinic.example.org, not an IP address)")
  valid_tls "$TLS_MODE" || missing+=("--tls letsencrypt|internal|tunnel")
  valid_email "$ADMIN_EMAIL" || missing+=("--admin-email")
  non_empty "$ORG_NAME" || missing+=("--org-name")
  [[ "$TLS_MODE" != tunnel || -n "$TUNNEL_TOKEN" ]] || missing+=("--tunnel-token (needed for --tls tunnel)")
  [[ ${#missing[@]} -eq 0 ]] || die "missing or invalid answers:" "${missing[@]}"

  check_disk

  if [[ -z "$VERSION" ]]; then
    VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ASD_DIR/../../rust/Cargo.toml" 2>/dev/null | head -n 1)
    VERSION=${VERSION:-latest}
  fi
  mkdir -p "$STATE/backups" "$STATE/drills"
  chmod 700 "$STATE" "$STATE/backups" "$STATE/drills"

  say "Creating the secrets"
  # Every secret is random and only ever written to the 0600 settings file. Values that
  # end up in URLs (passwords, MinIO keys) are hex so nothing needs escaping.
  cat >"$ASD_ENV_FILE" <<EOF
# Clinic server settings, written by install.sh on $(date -u +%Y-%m-%d). SECRET: mode 0600,
# never copy it into a backup, a ticket or a chat. Documentation: .env.example.
ASD_VERSION=$VERSION
ASD_IMAGE_PREFIX=${ASD_IMAGE_PREFIX:-}
DOMAIN=$DOMAIN
TLS_MODE=$TLS_MODE
SITE_SCHEME=$([[ "$TLS_MODE" == tunnel ]] && echo "http://")
PUBLISH_ADDR=$([[ "$TLS_MODE" == tunnel ]] && echo 127.0.0.1 || echo 0.0.0.0)
ADMIN_EMAIL=$ADMIN_EMAIL
ASD_STATE_DIR=$STATE
HTTP_PORT=${HTTP_PORT:-80}
HTTPS_PORT=${HTTPS_PORT:-443}
CLOUDFLARE_TUNNEL_TOKEN=$TUNNEL_TOKEN
POSTGRES_PASSWORD=$(rand_hex 24)
ASD_APP_DB_PASSWORD=$(rand_hex 24)
MINIO_ROOT_USER=root$(rand_hex 6)
MINIO_ROOT_PASSWORD=$(rand_hex 24)
MINIO_APP_ACCESS_KEY=app$(rand_hex 8)
MINIO_APP_SECRET_KEY=$(rand_hex 24)
ASD_DATA_KEK=$(rand_b64 32)
ASD_JWT_SIGNING_KEY=pending
ASD_BUNDLE_SIGNING_KEY=pending
ASD_BUNDLE_PUBLIC_KEY=pending
BACKUP_AGE_RECIPIENT=pending
BACKUP_KEY_FILE=/etc/asd/backup-key.txt
BACKUP_KEEP_DAILY=14
BACKUP_KEEP_WEEKLY=8
BACKUP_REMOTE=
EOF
  chmod 600 "$ASD_ENV_FILE"
  { printf 'OWNER_PASSWORD=%s\n' "$(rand_password)"; printf 'ORG_NAME=%s\n' "$ORG_NAME"; } >"$PENDING"
  chmod 600 "$PENDING"
  ok "settings written to $ASD_ENV_FILE (mode 0600)"
else
  say "Finishing an installation that stopped half-way"
fi

# ------------------------------------------------------------------ images and keys
ensure_images
VERSION=$(env_get ASD_VERSION)
PREFIX=$(env_get ASD_IMAGE_PREFIX)

if [[ "$(env_get ASD_JWT_SIGNING_KEY)" == pending || "$(env_get ASD_BUNDLE_SIGNING_KEY)" == pending ]]; then
  # `asd-cli bundle keygen`: PKCS#8 PEM on stdout, `public_key:` on stderr. The backend
  # also takes the bare 32-byte seed, which is the last 32 bytes of the PKCS#8 DER; the
  # seed is stored so .env stays one line per value.
  keygen() { # keygen -> "seed public_key"
    local pem info seed
    info=$(mktemp "$STATE/.keygen.XXXXXX")
    pem=$(docker run --rm --entrypoint /usr/local/bin/asd-cli "${PREFIX}asd-backend:$VERSION" bundle keygen 2>"$info") \
      || { rm -f "$info"; die "asd-cli bundle keygen failed."; }
    # PKCS#8 v1 or v2 (with the public key): the seed is the inner OCTET STRING 04 20.
    local hex
    hex=$(printf '%s\n' "$pem" | grep -v -- '-----' | tr -d '\n' | base64 -d | od -An -tx1 | tr -d ' \n')
    hex=$(sed -n 's/.*04220420\([0-9a-f]\{64\}\).*/\1/p' <<<"$hex")
    [[ ${#hex} -eq 64 ]] || { rm -f "$info"; die "asd-cli bundle keygen printed an unexpected key format."; }
    # shellcheck disable=SC2059,SC2001 # the format is the \xNN byte string built here
    seed=$(printf "$(sed 's/../\\x&/g' <<<"$hex")" | base64 | tr -d '\n')
    printf '%s %s' "$seed" "$(sed -n 's/^public_key: //p' "$info")"
    rm -f "$info"
  }
  read -r jwt _ <<<"$(keygen)"
  read -r bundle bundle_pub <<<"$(keygen)"
  [[ -n "$jwt" && -n "$bundle" && -n "$bundle_pub" ]] || die "creating the signing keys failed (see above)."
  env_set ASD_JWT_SIGNING_KEY "$jwt"
  env_set ASD_BUNDLE_SIGNING_KEY "$bundle"
  env_set ASD_BUNDLE_PUBLIC_KEY "$bundle_pub"
  ok "token and config-bundle signing keys created"
fi
if [[ "$(env_get BACKUP_AGE_RECIPIENT)" == pending ]]; then
  pair=$(docker run --rm --entrypoint age-keygen "${PREFIX}asd-backup:$VERSION" 2>/dev/null)
  env_set BACKUP_AGE_RECIPIENT "$(printf '%s\n' "$pair" | sed -n 's/^# public key: //p')"
  printf 'BACKUP_SECRET_KEY=%s\n' "$(printf '%s\n' "$pair" | grep '^AGE-SECRET-KEY-1')" >>"$PENDING"
  unset pair
  ok "backup key pair created (only the public half stays on this server)"
fi

# ------------------------------------------------------------------ start the stack
say "Starting the database and the file store"
dc up -d --wait postgres minio >/dev/null || die "the database or file store did not start." "Look at:  sudo docker compose -p $ASD_PROJECT logs postgres minio"
dc run --rm migrate >/dev/null || die "the database migration failed." "Look at the output above."
# DPL-4: api and worker log in as asd_app (created NOLOGIN by migration 0002).
printf "ALTER ROLE asd_app WITH LOGIN PASSWORD '%s';\n" "$(env_get ASD_APP_DB_PASSWORD)" \
  | dc exec -T postgres psql -v ON_ERROR_STOP=1 -qX -U asd -d asd >/dev/null \
  || die "could not set the asd_app database password."
ok "database ready; the API connects as asd_app"

say "Starting the clinic server"
dc up -d >/dev/null || die "the stack did not start." "Look at:  sudo $ASD_DIR/status.sh"
if [[ "$(env_get TLS_MODE)" == internal ]]; then
  for _ in $(seq 1 40); do
    dc exec -T caddy test -f /data/caddy/pki/authorities/local/root.crt 2>/dev/null && break
    sleep 2
  done
  dc cp caddy:/data/caddy/pki/authorities/local/root.crt "$STATE/caddy-root.crt" >/dev/null \
    || die "Caddy did not create its certificate authority." "Look at:  sudo docker compose -p $ASD_PROJECT logs caddy"
  chmod 644 "$STATE/caddy-root.crt"
fi
wait_ready 240 || die "the server did not answer on https://api.$(env_get DOMAIN)/health/ready." \
  "letsencrypt mode: the name must point at this server and ports 80/443 must be open." \
  "Look at:  sudo $ASD_DIR/status.sh"
ok "https://api.$(env_get DOMAIN)/health/ready answers through Caddy"

# ------------------------------------------------------------------ clinic + owner
say "Creating the clinic and its owner account"
ORG_NAME=$(pending_get ORG_NAME)
ADMIN_EMAIL=$(env_get ADMIN_EMAIL)
sql() { dc exec -T postgres psql -qAtX -v ON_ERROR_STOP=1 -U asd -d asd "$@"; }
org_id=$(sql -v name="$ORG_NAME" <<<"SELECT id FROM organizations WHERE lower(name) = lower(:'name') LIMIT 1;" 2>/dev/null || true)
if [[ -z "$org_id" ]]; then
  org_id=$(dc run --rm -T migrate admin create-org --name "$ORG_NAME" --kind clinic) \
    || die "could not create the clinic '$ORG_NAME'."
fi
user_exists=$(sql -v email="$ADMIN_EMAIL" <<<"SELECT 1 FROM users WHERE lower(email) = lower(:'email');" 2>/dev/null || true)
if [[ -z "$user_exists" ]]; then
  pending_get OWNER_PASSWORD | dc run --rm -T migrate admin create-user --email "$ADMIN_EMAIL" --role owner --org "$org_id" >/dev/null \
    || die "could not create the owner account $ADMIN_EMAIL."
fi
ok "clinic '$ORG_NAME' ($org_id), owner $ADMIN_EMAIL"

# ------------------------------------------------------------------ timers
if [[ -d /run/systemd/system ]] && command -v systemctl >/dev/null && [[ $EUID -eq 0 ]]; then
  for unit in asd-backup.service asd-backup.timer asd-drill.service asd-drill.timer; do
    sed -e "s#@ASD_DIR@#$ASD_DIR#g" -e "s#@ASD_ENV_FILE@#$ASD_ENV_FILE#g" \
      "$ASD_DIR/systemd/$unit" >"/etc/systemd/system/$unit"
    chmod 644 "/etc/systemd/system/$unit"
  done
  systemctl daemon-reload
  systemctl enable --now asd-backup.timer asd-drill.timer >/dev/null
  ok "nightly backup (02:30) and monthly restore drill (first Sunday 03:30) scheduled"
else
  warn "no systemd here: backup and drill timers NOT installed (run backup.sh / restore-drill.sh yourself)"
fi
mkdir -p /etc/asd 2>/dev/null && chmod 700 /etc/asd 2>/dev/null || true

# ------------------------------------------------------------------ key card
DOMAIN=$(env_get DOMAIN)
cat <<EOF

================================ KEY CARD =================================
 $ORG_NAME - $DOMAIN - installed $(date -u +%Y-%m-%d)

 Shown ONCE. Photograph it or copy it onto paper NOW and keep it locked away,
 OFF this server. Without it, backups cannot be restored and clinical notes
 cannot be read. Never e-mail it.

 Console:            https://$DOMAIN
 Device address:     https://api.$DOMAIN
 Owner sign-in:      $ADMIN_EMAIL
 Owner password:     $(pending_get OWNER_PASSWORD)
                     (set up the authenticator app within 14 days)
 DATA KEY:           $(env_get ASD_DATA_KEK)
 BACKUP KEY:         $(pending_get BACKUP_SECRET_KEY)
 Bundle public key:  $(env_get ASD_BUNDLE_PUBLIC_KEY)
===========================================================================
EOF
if [[ "$(env_get TLS_MODE)" == internal ]]; then
  echo " iPads must trust this certificate first: $STATE/caddy-root.crt (README.md, 'internal mode')."
fi
echo " To let the monthly restore drill run by itself, save the BACKUP KEY line in"
echo " $(env_get BACKUP_KEY_FILE) (README.md, 'Restore drill')."
rm -f "$PENDING"
say "Installation finished. Status any time:  sudo $ASD_DIR/status.sh"
