#!/usr/bin/env bash
# Builds the zip for a Windows milestone check (plan §11): fixture decks with
# reference PDFs and IR, the fonts to install and the checklist.
#
#   tools/milestone.sh phase0      (after make validate)
#
# Output: out/milestone/typst-pptx-PHASE-COMMIT/ and its zip. decks/ holds
# every subslide as its own slide (--animations slides): the owner exports
# each to PDF in PowerPoint as NAME-windows.pdf next to the checklist and
# tools/milestone_report.sh DIR compares them with Typst's pages. animated/
# holds the default exports of decks with click animations, checked by hand
# in the slideshow. A folder that already holds results is never
# overwritten.
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
  case $pptx in *.lo.pptx | *.slides.pptx) continue ;; esac
  name=$(basename "$pptx" .pptx)
  cp "out/validate/$name.slides.pptx" "$dir/decks/$name.pptx"
  cp "out/validate/$name.slides.ir.json" "$dir/decks/$name.ir.json"
  cp "out/validate/$name.pdf" "$dir/decks/"
  if grep -q '"clicks"\|"transition"' "out/validate/$name.ir.json"; then
    mkdir -p "$dir/animated"
    cp "$pptx" "$dir/animated/"
  fi
done
cp fixtures/fonts/ibm-plex/*.ttf fixtures/fonts/ibm-plex/OFL.txt "$dir/fonts/"
cp "$checklist" "$dir/checklist.md"
echo "$commit" > "$dir/COMMIT"

(cd out/milestone && zip -qr "$(basename "$dir").zip" "$(basename "$dir")")
ls -la "$dir.zip"
