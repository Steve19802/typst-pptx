#!/usr/bin/env bash
# Builds the zip for a Windows milestone check (plan §11): fixture decks with
# reference PDFs and IR, the fonts to install and the checklist.
#
#   tools/milestone.sh phase0      (after make validate)
#
# Output: out/milestone/typst-pptx-PHASE-COMMIT/ and its zip. The owner
# exports each deck to PDF in PowerPoint as NAME-windows.pdf next to the
# checklist; tools/milestone_report.sh DIR then compares them. A folder that
# already holds results is never overwritten.
set -euo pipefail
cd "$(dirname "$0")/.."

phase=${1:?usage: tools/milestone.sh PHASE}
checklist=docs/milestones/$phase-checklist.md
[ -f "$checklist" ] || { echo "missing $checklist" >&2; exit 1; }
[ -d out/validate ] || { echo "run make validate first" >&2; exit 1; }

commit=$(git rev-parse --short HEAD 2>/dev/null || echo nogit)
if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
  commit=$commit-dirty
fi
dir=out/milestone/typst-pptx-$phase-$commit
if compgen -G "$dir/*-windows.pdf" >/dev/null; then
  echo "$dir already holds PowerPoint results; commit first or move them" >&2
  exit 1
fi
rm -rf "$dir" "$dir.zip"
mkdir -p "$dir/decks" "$dir/fonts"

for pptx in out/validate/*.pptx; do
  case $pptx in *.lo.pptx) continue ;; esac
  name=$(basename "$pptx" .pptx)
  cp "$pptx" "out/validate/$name.pdf" "out/validate/$name.ir.json" "$dir/decks/"
done
cp fixtures/fonts/ibm-plex/*.ttf fixtures/fonts/ibm-plex/OFL.txt "$dir/fonts/"
cp "$checklist" "$dir/checklist.md"
echo "$commit" > "$dir/COMMIT"

(cd out/milestone && zip -qr "$(basename "$dir").zip" "$(basename "$dir")")
ls -la "$dir.zip"
