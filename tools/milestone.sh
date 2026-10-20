#!/usr/bin/env bash
# Builds the zip for a Windows milestone check (plan §11): fixture decks
# with reference PDFs, the baseline calibration deck, the fonts to install
# and the checklist.
#
#   tools/milestone.sh phase0      (after make validate)
set -euo pipefail
cd "$(dirname "$0")/.."

phase=${1:?usage: tools/milestone.sh PHASE}
checklist=docs/milestones/$phase-checklist.md
[ -f "$checklist" ] || { echo "missing $checklist" >&2; exit 1; }
[ -d out/validate ] || { echo "run make validate first" >&2; exit 1; }
assets=$(ls -d "$HOME"/.cargo/registry/src/*/typst-assets-0.15.1/files/fonts | head -1)

dir=out/milestone/typst-pptx-$phase
rm -rf "$dir" "$dir.zip"
mkdir -p "$dir/decks" "$dir/fonts"

for name in decks-features decks-touying-basic signal-wave-demo; do
  cp "out/validate/$name.pptx" "out/validate/$name.pdf" "$dir/decks/"
done
uv run -q --project tools/validate python -I tools/validate/calibrate_baseline.py \
  out/calibration --extra-fonts "$assets" >/dev/null
cp out/calibration/calibration.pptx "$dir/decks/"

cp fixtures/fonts/ibm-plex/*.ttf fixtures/fonts/ibm-plex/OFL.txt "$dir/fonts/"
cp "$assets"/LibertinusSerif-Regular.otf "$assets"/NewCM10-Regular.otf \
  "$assets"/DejaVuSansMono.ttf "$dir/fonts/"
cp "$checklist" "$dir/checklist.md"
git rev-parse --short HEAD > "$dir/COMMIT" 2>/dev/null || true

(cd out/milestone && zip -qr "typst-pptx-$phase.zip" "typst-pptx-$phase")
ls -la "$dir.zip"
