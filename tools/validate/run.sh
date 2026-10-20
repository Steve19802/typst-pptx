#!/usr/bin/env bash
# Automatic package and visual checks over the fixture decks (plan §11):
# Open XML SDK validator, python-pptx open check, LibreOffice rendering
# compared with Typst's PDF, text line positions. Offline: fonts and packages
# from fixtures/ only.
#
# Each deck is exported twice: NAME.pptx with the default PowerPoint text
# model (validated, shipped in milestone zips) and NAME.lo.pptx with
# --text-model libreoffice, so LibreOffice's rendering can be compared with
# Typst's exactly.
#
#   tools/validate/run.sh            (after cargo build --release)
#   MIN_SSIM=0.9 tools/validate/run.sh
set -euo pipefail
cd "$(dirname "$0")/../.."

BIN=${BIN:-target/release/typst-pptx}
OUT=${OUT:-out/validate}
MIN_SSIM=${MIN_SSIM:-0.94}
MAX_DY=${MAX_DY:-0.5}
MAX_DW=${MAX_DW:-2}
DECKS=(
  fixtures/decks/features.typ
  fixtures/decks/touying-basic.typ
  fixtures/decks/text-metrics.typ
  fixtures/themes/signal-wave/demo.typ
)
FLAGS=(
  --root .
  --font-path fixtures/fonts
  --ignore-system-fonts
  --package-path fixtures/packages
  --package-cache-path "$OUT/no-package-cache"
  --creation-timestamp 1767225600
)

rm -rf "$OUT"
mkdir -p "$OUT/lo"
pptx=()
for deck in "${DECKS[@]}"; do
  name=$(basename "$(dirname "$deck")")-$(basename "$deck" .typ)
  echo "== $name"
  typst compile "${FLAGS[@]}" "$deck" "$OUT/$name.pdf" 2>/dev/null
  "$BIN" compile "${FLAGS[@]}" --diagnostic-format short "$deck" "$OUT/$name.pptx" \
    --emit-ir "$OUT/$name.ir.json" 2>&1 | grep -v "unknown font family" || true
  "$BIN" compile "${FLAGS[@]}" --diagnostic-format short --text-model libreoffice "$deck" \
    "$OUT/$name.lo.pptx" --emit-ir "$OUT/$name.lo.ir.json" 2>/dev/null
  pptx+=("$OUT/$name.pptx")
done

echo "== Open XML SDK validator"
dotnet run --project tools/validate/oxml -c Release -- "${pptx[@]}" 2>&1 | grep -v workloads

echo "== python-pptx"
uv run -q --project tools/validate python -I -c '
import sys, pptx
for path in sys.argv[1:]:
    deck = pptx.Presentation(path)
    notes = sum(1 for slide in deck.slides if slide.has_notes_slide)
    print(f"{path}: {len(deck.slides)} slides, {notes} with notes")
' "${pptx[@]}"

echo "== LibreOffice render vs Typst (min SSIM $MIN_SSIM, max line offset $MAX_DY pt, width $MAX_DW pt)"
soffice --headless --convert-to pdf --outdir "$OUT/lo" "${pptx[@]/%.pptx/.lo.pptx}" >/dev/null 2>&1
status=0
for file in "${pptx[@]}"; do
  name=$(basename "$file" .pptx)
  echo "-- $name"
  uv run -q --project tools/validate python -I tools/validate/compare.py \
    "$OUT/$name.pdf" "$OUT/lo/$name.lo.pdf" "$OUT/compare/$name" --dpi 60 \
    --min-ssim "$MIN_SSIM" | tr "\n" " " || status=1
  echo
  uv run -q --project tools/validate python -I tools/validate/text_positions.py \
    "$OUT/$name.lo.ir.json" "$OUT/$name.pdf" "$OUT/lo/$name.lo.pdf" \
    --max-dy "$MAX_DY" --max-dw "$MAX_DW" | tail -2 || status=1
done
exit $status
