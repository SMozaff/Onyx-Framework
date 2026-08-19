#!/usr/bin/env bash
# Installs and verifies a prebuilt ONYX api-server on Debian 13 (trixie).
#
# This script expects a trusted api-server Linux executable already downloaded
# from an ONYX release. It uses SQLite for a single-server/LAN setup, creates a
# low-privilege `onyx` account, verifies /health and a genuine login, and opens
# UFW only when UFW is both installed and active.
#
# Default behavior starts a regular background process. Pass --install-service
# to create and enable a systemd unit, which survives logout/reboot and
# automatically restarts the service after failures.
#
# Example:
#   sudo ./setup-onyx-debian.sh --api-server /home/admin/api-server \
#     --admin-username admin
#
# Existing databases are protected by default. To preserve one and only verify
# a known Admin's credentials, add --reuse-existing-database.

set -Eeuo pipefail
IFS=$'\n\t'

INSTALL_ROOT="/opt/onyx"
DATA_DIR="/var/lib/onyx"
LOG_DIR="/var/log/onyx"
UNIT_PATH="/etc/systemd/system/onyx-api.service"
API_SOURCE=""
BIND="0.0.0.0:3000"
METRICS_BIND="127.0.0.1:9090"
DATABASE_PATH=""
ADMIN_USERNAME="admin"
ADMIN_PASSWORD=""
BOOTSTRAP_TOKEN=""
INSTALL_SERVICE=0
REUSE_EXISTING_DATABASE=0
ALLOW_PUBLIC_NETWORK=0

step() { printf '\n==> %s\n' "$1"; }
ok() { printf '    [OK] %s\n' "$1"; }
fail() { printf '    [FAILED] %s\n' "$1" >&2; exit 1; }
usage() {
  cat <<'USAGE'
Usage:
  sudo ./setup-onyx-debian.sh --api-server /path/to/api-server [options]

Required:
  --api-server PATH              Trusted prebuilt ONYX api-server executable.

Options:
  --install-root PATH            Installation directory (default: /opt/onyx).
  --bind ADDRESS:PORT            HTTP/relay bind (default: 0.0.0.0:3000).
  --metrics-bind ADDRESS:PORT    Metrics bind (default: 127.0.0.1:9090).
  --database PATH                SQLite file path (default: /var/lib/onyx/onyx.db).
  --admin-username USER          First/existing Admin username (default: admin).
  --admin-password PASSWORD      Password. If omitted, prompts without echoing.
  --bootstrap-token TOKEN        One-time token. If omitted for a new database,
                                  a cryptographically random token is generated.
  --reuse-existing-database      Do not create an Admin; verify the given existing
                                  Admin credentials against the running server.
  --install-service              Create/enable a systemd service (opt-in).
  --allow-public-network         If active, open UFW TCP port from any address.
                                  Default UFW rule is restricted to private ranges.
  -h, --help                     Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --api-server) API_SOURCE=${2:?"--api-server needs a path"}; shift 2 ;;
    --install-root) INSTALL_ROOT=${2:?"--install-root needs a path"}; shift 2 ;;
    --bind) BIND=${2:?"--bind needs ADDRESS:PORT"}; shift 2 ;;
    --metrics-bind) METRICS_BIND=${2:?"--metrics-bind needs ADDRESS:PORT"}; shift 2 ;;
    --database) DATABASE_PATH=${2:?"--database needs a path"}; shift 2 ;;
    --admin-username) ADMIN_USERNAME=${2:?"--admin-username needs a value"}; shift 2 ;;
    --admin-password) ADMIN_PASSWORD=${2:?"--admin-password needs a value"}; shift 2 ;;
    --bootstrap-token) BOOTSTRAP_TOKEN=${2:?"--bootstrap-token needs a value"}; shift 2 ;;
    --reuse-existing-database) REUSE_EXISTING_DATABASE=1; shift ;;
    --install-service) INSTALL_SERVICE=1; shift ;;
    --allow-public-network) ALLOW_PUBLIC_NETWORK=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) fail "Unknown argument: $1" ;;
  esac
done

[[ $EUID -eq 0 ]] || fail "Run this script with sudo/root privileges."
[[ -n "$API_SOURCE" ]] || { usage; fail "--api-server is required."; }
[[ -f "$API_SOURCE" ]] || fail "api-server binary not found: $API_SOURCE"
[[ -x "$API_SOURCE" ]] || fail "api-server binary is not executable: $API_SOURCE"
[[ "$BIND" =~ ^[^:]+:[0-9]{1,5}$ ]] || fail "--bind must be ADDRESS:PORT (IPv4/hostname form)."
[[ "$METRICS_BIND" =~ ^[^:]+:[0-9]{1,5}$ ]] || fail "--metrics-bind must be ADDRESS:PORT (IPv4/hostname form)."
[[ "$ADMIN_USERNAME" =~ ^[A-Za-z0-9._-]+$ ]] || fail "Admin usernames may contain only letters, digits, dot, underscore, or hyphen."

API_PORT=${BIND##*:}
(( API_PORT >= 1 && API_PORT <= 65535 )) || fail "API port must be between 1 and 65535."
if [[ -z "$DATABASE_PATH" ]]; then
  DATABASE_PATH="$DATA_DIR/onyx.db"
fi
if [[ -e "$DATABASE_PATH" && $REUSE_EXISTING_DATABASE -eq 0 ]]; then
  fail "Database already exists at $DATABASE_PATH. Re-run with --reuse-existing-database to preserve it and verify an existing Admin login."
fi
if [[ ! -e "$DATABASE_PATH" && $REUSE_EXISTING_DATABASE -eq 1 ]]; then
  fail "--reuse-existing-database was supplied but no database exists at $DATABASE_PATH."
fi
if [[ $REUSE_EXISTING_DATABASE -eq 0 && -z "$BOOTSTRAP_TOKEN" ]]; then
  BOOTSTRAP_TOKEN=$(openssl rand -base64 32 | tr '+/' '-_' | tr -d '=')
fi
if [[ -z "$ADMIN_PASSWORD" ]]; then
  read -r -s -p "Password for '$ADMIN_USERNAME' (not echoed or logged): " ADMIN_PASSWORD
  printf '\n'
fi
[[ -n "$ADMIN_PASSWORD" ]] || fail "An Admin password is required."

if command -v ss >/dev/null 2>&1 && ss -ltn "sport = :$API_PORT" | grep -q LISTEN; then
  fail "Port $API_PORT is already listening. Stop the conflicting process before setup."
fi

API_BIN="$INSTALL_ROOT/bin/api-server"
ENV_FILE="$INSTALL_ROOT/onyx-api.env"
DATABASE_DIR=$(dirname "$DATABASE_PATH")
DATABASE_URL="sqlite:///${DATABASE_PATH#/}?mode=rwc"

ensure_dependencies() {
  step "Installing runtime verification dependencies"
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y --no-install-recommends ca-certificates curl jq openssl
  ok "Installed ca-certificates, curl, jq, and openssl"
}

write_environment_file() {
  local token=${1:-}
  umask 077
  {
    printf 'ONYX_BIND=%s\n' "$BIND"
    printf 'ONYX_METRICS_BIND=%s\n' "$METRICS_BIND"
    printf 'DATABASE_URL=%s\n' "$DATABASE_URL"
    printf 'ONYX_ENV=development\n'
    if [[ -n "$token" ]]; then
      printf 'ONYX_BOOTSTRAP_TOKEN=%s\n' "$token"
    fi
  } > "$ENV_FILE"
  chown root:onyx "$ENV_FILE"
  chmod 640 "$ENV_FILE"
}

start_direct_process() {
  local token=${1:-}
  write_environment_file "$token"
  local -a process_environment=(
    "ONYX_BIND=$BIND"
    "ONYX_METRICS_BIND=$METRICS_BIND"
    "DATABASE_URL=$DATABASE_URL"
    "ONYX_ENV=development"
  )
  if [[ -n "$token" ]]; then
    process_environment+=("ONYX_BOOTSTRAP_TOKEN=$token")
  fi
  runuser -u onyx -- env "${process_environment[@]}" \
    nohup "$API_BIN" >> "$LOG_DIR/api-server.log" 2>&1 < /dev/null &
  API_PID=$!
  sleep 0.3
  if ! kill -0 "$API_PID" 2>/dev/null; then
    fail "api-server exited immediately. Check $LOG_DIR/api-server.log"
  fi
}

stop_direct_process() {
  if [[ -n ${API_PID:-} ]] && kill -0 "$API_PID" 2>/dev/null; then
    kill "$API_PID" || true
    wait "$API_PID" 2>/dev/null || true
  fi
}

wait_for_health() {
  local server_url=$1
  for _ in $(seq 1 30); do
    if curl --fail --silent --max-time 2 "$server_url/health" | jq -e '.status == "ok"' >/dev/null 2>&1; then
      ok "Health check passed at $server_url/health"
      return 0
    fi
    sleep 1
  done
  fail "The API did not become healthy within 30 seconds. Check $LOG_DIR/api-server.log"
}

bootstrap_admin() {
  local server_url=$1
  local response
  response=$(curl --fail --silent --show-error --max-time 10 \
    -X POST "$server_url/api/admin/bootstrap" \
    -H "content-type: application/json" \
    -H "x-onyx-bootstrap-token: $BOOTSTRAP_TOKEN" \
    --data "$(jq -cn --arg username "$ADMIN_USERNAME" --arg password "$ADMIN_PASSWORD" '{username:$username,password:$password,is_admin:true}')") \
    || fail "First-admin bootstrap request failed."
  jq -e --arg username "$ADMIN_USERNAME" '.username == $username and .is_admin == true' <<<"$response" >/dev/null \
    || fail "Bootstrap returned an unexpected user record."
  ok "Created first Admin account '$ADMIN_USERNAME'"
}

verify_login() {
  local server_url=$1
  local response
  response=$(curl --fail --silent --show-error --max-time 10 \
    -X POST "$server_url/api/auth/login" -H "content-type: application/json" \
    --data "$(jq -cn --arg username "$ADMIN_USERNAME" --arg password "$ADMIN_PASSWORD" '{username:$username,password:$password}')") \
    || fail "Login verification request failed."
  jq -e --arg username "$ADMIN_USERNAME" '.access_token | strings | length > 0' <<<"$response" >/dev/null \
    || fail "Login response did not include an access token."
  jq -e --arg username "$ADMIN_USERNAME" '.user.username == $username' <<<"$response" >/dev/null \
    || fail "Login response did not identify the expected Admin user."
  ok "Real login verification passed for '$ADMIN_USERNAME'"
}

configure_ufw_if_active() {
  step "Checking UFW firewall state"
  if ! command -v ufw >/dev/null 2>&1; then
    printf '    [SKIP] UFW is not installed; no firewall configuration was changed.\n'
    return
  fi
  if ! ufw status | head -n 1 | grep -qx 'Status: active'; then
    printf '    [SKIP] UFW is installed but inactive; no firewall configuration was changed.\n'
    return
  fi
  if [[ $ALLOW_PUBLIC_NETWORK -eq 1 ]]; then
    ufw allow "$API_PORT/tcp"
    ok "UFW allows TCP/$API_PORT from any address (explicit public-network opt-in)"
  else
    for source in 10.0.0.0/8 172.16.0.0/12 192.168.0.0/16; do
      ufw allow from "$source" to any port "$API_PORT" proto tcp
    done
    ok "UFW allows TCP/$API_PORT from private IPv4 ranges"
  fi
}

install_systemd_service() {
  step "Installing the opt-in systemd service"
  cat > "$UNIT_PATH" <<UNIT
[Unit]
Description=ONYX API Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=onyx
Group=onyx
WorkingDirectory=$INSTALL_ROOT
EnvironmentFile=$ENV_FILE
ExecStart=$API_BIN
Restart=on-failure
RestartSec=5
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=$DATABASE_DIR $LOG_DIR

[Install]
WantedBy=multi-user.target
UNIT
  systemctl daemon-reload
  systemctl enable --now onyx-api.service
  systemctl is-active --quiet onyx-api.service || fail "systemd did not report onyx-api.service as active."
  ok "Enabled onyx-api.service; it will restart on failure and start after reboot"
}

restart_service_with_current_environment() {
  systemctl restart onyx-api.service
  systemctl is-active --quiet onyx-api.service || fail "ONYX systemd service did not restart successfully."
}

ensure_dependencies

step "Creating low-privilege service account and installation directories"
if ! id -u onyx >/dev/null 2>&1; then
  useradd --system --home-dir "$DATA_DIR" --create-home --shell /usr/sbin/nologin onyx
fi
install -d -o root -g root -m 0755 "$INSTALL_ROOT/bin"
install -d -o onyx -g onyx -m 0750 "$DATABASE_DIR" "$LOG_DIR"
install -o root -g root -m 0755 "$API_SOURCE" "$API_BIN"
ok "Installed api-server at $API_BIN"

configure_ufw_if_active

if [[ $INSTALL_SERVICE -eq 1 ]]; then
  write_environment_file "$([[ $REUSE_EXISTING_DATABASE -eq 1 ]] && printf '' || printf '%s' "$BOOTSTRAP_TOKEN")"
  install_systemd_service
else
  step "Starting api-server as a regular background process"
  start_direct_process "$([[ $REUSE_EXISTING_DATABASE -eq 1 ]] && printf '' || printf '%s' "$BOOTSTRAP_TOKEN")"
  ok "api-server started as PID $API_PID; it will not automatically restart or survive a reboot without --install-service"
fi

SERVER_URL="http://127.0.0.1:$API_PORT"
cleanup_password() {
  ADMIN_PASSWORD=""
  BOOTSTRAP_TOKEN=""
}
trap cleanup_password EXIT

wait_for_health "$SERVER_URL"
if [[ $REUSE_EXISTING_DATABASE -eq 0 ]]; then
  step "Bootstrapping the first Admin account"
  bootstrap_admin "$SERVER_URL"

  # Clear the one-time bootstrap token after it has created the only allowed
  # first account, then prove the server remains healthy under normal settings.
  if [[ $INSTALL_SERVICE -eq 1 ]]; then
    write_environment_file ""
    restart_service_with_current_environment
  else
    stop_direct_process
    start_direct_process ""
  fi
  wait_for_health "$SERVER_URL"
fi

step "Verifying a real Admin login"
verify_login "$SERVER_URL"

printf '\nSetup complete. This PC can use %s. Other LAN clients should use http://<this-PCs-LAN-IP>:%s.\n' "$SERVER_URL" "$API_PORT"
if [[ $INSTALL_SERVICE -eq 0 ]]; then
  printf 'The API is a regular background process. Re-run with --install-service when you need automatic restart after logout or reboot.\n'
fi
