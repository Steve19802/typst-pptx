#!/usr/bin/env bash
# Compares PowerPoint's PDF exports from a milestone check with Typst's PDFs:
# visual diff per slide and text line positions (plan §11).
#
#   tools/milestone_report.sh out/milestone/typst-pptx-phase0-abc1234
#
# Expects DIR/NAME-windows.pdf for the decks in DIR/decks/. Writes
# side-by-side images to DIR/report/.
set -euo pipefail
cd "$(dirname "$0")/.."

dir=${1:?usage: tools/milestone_report.sh DIR}
report=$dir/report
[ -d "$dir" ] || { echo "missing $dir" >&2; exit 1; }
cat "$dir"/*version*.txt 2>/dev/null || true

for pptx in "$dir"/decks/*.pptx; do
  name=$(basename "$pptx" .pptx)
  windows=$dir/$name-windows.pdf
  if [ ! -f "$windows" ]; then
    echo "-- $name: no $name-windows.pdf, skipped"
    continue
  fi
  echo "-- $name"
  uv run -q --project tools/validate python -I tools/validate/compare.py \
    "$dir/decks/$name.pdf" "$windows" "$report/$name" --dpi 60 | tr "\n" " "
  echo
  if [ -f "$dir/decks/$name.ir.json" ]; then
    uv run -q --project tools/validate python -I tools/validate/text_positions.py \
      "$dir/decks/$name.ir.json" "$dir/decks/$name.pdf" "$windows"
  fi
done
