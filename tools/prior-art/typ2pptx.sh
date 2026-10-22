#!/usr/bin/env bash
# Comparison with typ2pptx (docs/prior-art.md): converts the fixture decks
# with both tools, renders them with LibreOffice and writes side-by-side
# sheets (Typst | typst-pptx | typ2pptx) and structure stats to
# out/scratch/t2p/out. Run in the devcontainer after `cargo build --release`.
#
# typ2pptx bundles Typst 0.14, which can't load Touying 0.8, so its inputs are
# copies pinned to Touying 0.7.4.
set -euo pipefail
cd "$(dirname "$0")/../.."
root=$PWD
version=${TYP2PPTX_VERSION:-0.2.3}
w=$root/out/scratch/t2p
out=$w/out
rm -rf "$w/src" "$out"
mkdir -p "$w/data/typst" "$w/src" "$out"

test -x "$w/venv/bin/typ2pptx" || uv venv -q "$w/venv"
uv pip install -q --python "$w/venv" "typ2pptx==$version"

# typst-ts-cli has no --package-path: it reads $XDG_DATA_HOME/typst/packages.
ln -sfn "$root/fixtures/packages" "$w/data/typst/packages"
export XDG_DATA_HOME=$w/data TYPST_FONT_PATHS=$root/fixtures/fonts

cp fixtures/decks/features.typ fixtures/decks/tables.typ "$w/src/"
cp -r fixtures/decks/assets "$w/src/"
cp fixtures/themes/signal-wave/*.typ "$w/src/"
cp -r fixtures/themes/signal-wave/assets "$w/src/" 2>/dev/null || true
sed -i 's/touying:0\.8\.0/touying:0.7.4/' "$w/src"/*.typ

flags=(--font-path fixtures/fonts --ignore-system-fonts --package-path fixtures/packages)
for deck in features tables demo; do
  case $deck in
    demo) src=fixtures/themes/signal-wave/demo.typ ;;
    *) src=fixtures/decks/$deck.typ ;;
  esac
  typst compile --root . "${flags[@]}" "$src" "$out/$deck-typst.pdf" 2>/dev/null
  target/release/typst-pptx compile --root . "${flags[@]}" --package-cache-path /nonexistent \
    "$src" "$out/$deck-typst-pptx.pptx" 2>/dev/null
  (cd "$w/src" && "$w/venv/bin/typ2pptx" "$deck.typ" -o "$out/$deck-typ2pptx.pptx" > "$out/$deck-typ2pptx.log" 2>&1)
done
rm -f "$w/src"/*.artifact.svg

(cd "$out" && for f in *.pptx; do soffice --headless --convert-to pdf "$f" > /dev/null 2>&1; done)
py=(uv run -q --project tools/validate python -I)
"${py[@]}" tools/prior-art/pptx_stats.py "$out"/*.pptx | tee "$out/stats.txt"
for spec in "features 2 3" "tables 1" "demo 5 10 14"; do
  set -- $spec
  deck=$1; shift
  "${py[@]}" tools/prior-art/sheet.py "$out/$deck" "$out/$deck-typst.pdf" \
    "$out/$deck-typst-pptx.pdf" "$out/$deck-typ2pptx.pdf" "$@"
done
ls "$out"/*.png
