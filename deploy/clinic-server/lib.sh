# shellcheck shell=bash
# owner: WP-B.18 (DPL-1)
#
# Shared helpers for the clinic-server scripts. Sourced, never run. Messages are written
# for the person running the server (often not an engineer): say what failed and what to
# do next, never print a secret.
#
# Environment knobs shared by every script (all optional):
#   ASD_ENV_FILE        settings file            (default: <this folder>/.env)
#   ASD_PROJECT         compose project name     (default: asd-clinic)
#   ASD_DRILL_PROJECT   scratch drill project    (default: asd-drill)

ASD_DIR="${ASD_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"
ASD_ENV_FILE="${ASD_ENV_FILE:-$ASD_DIR/.env}"
ASD_PROJECT="${ASD_PROJECT:-asd-clinic}"
ASD_DRILL_PROJECT="${ASD_DRILL_PROJECT:-asd-drill}"

if [[ -t 1 ]]; then
  _B=$'\033[1m'; _R=$'\033[31m'; _G=$'\033[32m'; _Y=$'\033[33m'; _N=$'\033[0m'
else
  _B=""; _R=""; _G=""; _Y=""; _N=""
fi

say()  { printf '%s==> %s%s\n' "$_B" "$*" "$_N"; }
ok()   { printf '    %sOK%s    %s\n' "$_G" "$_N" "$*"; }
warn() { printf '    %sWARN%s  %s\n' "$_Y" "$_N" "$*" >&2; }
die()  {
  printf '\n%sERROR:%s %s\n' "$_R" "$_N" "$1" >&2
  shift
  local line
  for line in "$@"; do printf '       %s\n' "$line" >&2; done
  exit 1
}

# env_get KEY [FILE]: the value of KEY in the settings file (no shell evaluation; the file
# is never sourced, so a value with `$` or quotes cannot run anything).
env_get() {
  local file="${2:-$ASD_ENV_FILE}" line
  [[ -r "$file" ]] || return 0
  line=$(grep -E "^$1=" "$file" | tail -n 1) || return 0
  line="${line#*=}"
  line="${line%\"}"; line="${line#\"}"
  printf '%s' "$line"
}

# env_set KEY VALUE [FILE]: replace or append one line, keeping the file mode.
env_set() {
  local key=$1 value=$2 file="${3:-$ASD_ENV_FILE}" tmp
  tmp=$(mktemp "${file}.XXXXXX")
  chmod 600 "$tmp"
  grep -vE "^$key=" "$file" >"$tmp" || true
  printf '%s=%s\n' "$key" "$value" >>"$tmp"
  mv "$tmp" "$file"
}

require_env_file() {
  [[ -f "$ASD_ENV_FILE" ]] || die "no settings file at $ASD_ENV_FILE." \
    "This server has not been installed yet. Run:  sudo $ASD_DIR/install.sh"
}

state_dir() {
  local d
  d=$(env_get ASD_STATE_DIR)
  printf '%s' "${d:-/var/lib/asd}"
}

# dc ARGS...: docker compose for the production project, with the settings file.
dc() {
  local profiles=()
  [[ "$(env_get TLS_MODE)" == tunnel ]] && profiles+=(--profile tunnel)
  docker compose --project-directory "$ASD_DIR" -f "$ASD_DIR/docker-compose.yml" \
    --env-file "$ASD_ENV_FILE" -p "$ASD_PROJECT" ${profiles[@]+"${profiles[@]}"} "$@"
}

# dc_drill ARGS...: the same file as the scratch project used by restore-drill.sh.
dc_drill() {
  docker compose --project-directory "$ASD_DIR" -f "$ASD_DIR/docker-compose.yml" \
    --env-file "$ASD_ENV_FILE" -p "$ASD_DRILL_PROJECT" "$@"
}

require_root() {
  if [[ $EUID -ne 0 && -z "${ASD_DEV_NON_ROOT:-}" ]]; then
    die "this script must run as the administrator (root)." \
      "Run it again with sudo in front, for example:  sudo $0"
  fi
}

require_docker() {
  command -v docker >/dev/null 2>&1 || die "Docker is not installed." \
    "Install Docker Engine 25 or newer with the compose plugin:" \
    "https://docs.docker.com/engine/install/ubuntu/  (then run this script again)"
  docker compose version >/dev/null 2>&1 || die "the Docker compose plugin is missing." \
    "Install it with:  sudo apt-get install docker-compose-plugin"
  docker info >/dev/null 2>&1 || die "Docker is installed but not running (or this user may not use it)." \
    "Start it with:  sudo systemctl start docker"
  local v major
  v=$(docker version --format '{{.Server.Version}}' 2>/dev/null || echo 0)
  major=${v%%.*}
  [[ "$major" =~ ^[0-9]+$ && "$major" -ge 25 ]] || die "Docker $v is too old; version 25 or newer is needed." \
    "Upgrade Docker Engine: https://docs.docker.com/engine/install/ubuntu/"
}

# curl_site URL [curl args...]: curl to this stack through Caddy without needing DNS: the
# domain is pinned to 127.0.0.1 and, in `internal` TLS mode, verified against Caddy's root.
curl_site() {
  local url=$1; shift
  local host port args=()
  host=${url#https://}; host=${host%%/*}; host=${host%%:*}
  port=$(env_get HTTPS_PORT); port=${port:-443}
  case "$(env_get TLS_MODE)" in
    tunnel)
      port=$(env_get HTTP_PORT); port=${port:-80}
      url=${url/https:\/\//http://}
      url=${url/$host/$host:$port}
      ;;
    internal)
      local ca; ca="$(state_dir)/caddy-root.crt"
      if [[ -r "$ca" ]]; then args+=(--cacert "$ca"); else args+=(-k); fi
      url=${url/$host/$host:$port}
      ;;
    *) url=${url/$host/$host:$port} ;;
  esac
  curl -sS --max-time 10 --resolve "$host:$port:127.0.0.1" ${args[@]+"${args[@]}"} "$@" "$url"
}

# wait_ready SECONDS: until https://api.<domain>/health/ready answers 200 through Caddy.
wait_ready() {
  local domain i
  domain=$(env_get DOMAIN)
  for ((i = 0; i < $1; i += 3)); do
    if curl_site "https://api.$domain/health/ready" -fo /dev/null 2>/dev/null; then return 0; fi
    sleep 3
  done
  return 1
}

# backend_version: the version the running API reports on /health.
backend_version() {
  curl_site "https://api.$(env_get DOMAIN)/health" 2>/dev/null \
    | sed -n 's/.*"version":"\([^"]*\)".*/\1/p'
}
