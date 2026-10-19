#!/usr/bin/env bash
# Named volumes may be root-owned (new target/ volume) or carry the image's
# original UID (after updateRemoteUserUID); hand them all to the user.
set -euo pipefail

sudo chown -R "$(id -u):$(id -g)" \
  "$HOME/.cargo/registry" "$HOME/.cargo/git" "$HOME/.cache" "$HOME/.nuget" \
  /workspaces/typst-pptx/target

mkdir -p /workspaces/typst-pptx/fixtures/fonts /workspaces/typst-pptx/out
fc-cache -f >/dev/null
