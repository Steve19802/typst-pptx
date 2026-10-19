#!/usr/bin/env bash
# Starts a non-root sshd on port 2222 for the agent container.
# Keys live in .devcontainer/ssh/ (gitignored) so they survive rebuilds:
#   authorized_keys   public keys allowed to log in
#   host/             sshd host key, generated on first start
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
keys="$repo/.devcontainer/ssh"
state="$HOME/.ssh-server"

if [ ! -s "$keys/authorized_keys" ]; then
  echo "start-sshd: $keys/authorized_keys is missing or empty, sshd not started" >&2
  exit 0
fi

mkdir -p "$keys/host" "$state" "$HOME/.ssh"
chmod 700 "$state" "$HOME/.ssh"
if [ ! -f "$keys/host/ssh_host_ed25519_key" ]; then
  ssh-keygen -q -t ed25519 -N '' -C typst-pptx-dev -f "$keys/host/ssh_host_ed25519_key"
fi
install -m 600 "$keys/host/ssh_host_ed25519_key" "$state/ssh_host_ed25519_key"
install -m 600 "$keys/authorized_keys" "$HOME/.ssh/authorized_keys"

# SSH sessions don't inherit the image's ENV; hand it over explicitly.
: > "$HOME/.ssh/environment"
for var in PATH LANG CARGO_HOME RUSTUP_HOME DOTNET_ROOT DOTNET_CLI_TELEMETRY_OPTOUT \
           DOTNET_NOLOGO UV_LINK_MODE TYPST_FONT_PATHS TYPST_IGNORE_SYSTEM_FONTS; do
  if [ -n "${!var:-}" ]; then
    printf '%s=%s\n' "$var" "${!var}" >> "$HOME/.ssh/environment"
  fi
done
chmod 600 "$HOME/.ssh/environment"

pidfile=/tmp/typst-pptx-sshd.pid
if [ -f "$pidfile" ] && kill -0 "$(cat "$pidfile")" 2>/dev/null; then
  kill "$(cat "$pidfile")"
  sleep 0.5
fi

/usr/sbin/sshd -f /etc/typst-pptx/sshd_config \
  -h "$state/ssh_host_ed25519_key" \
  -o "AllowUsers=$(id -un)"
echo "start-sshd: listening on $(hostname):2222"
