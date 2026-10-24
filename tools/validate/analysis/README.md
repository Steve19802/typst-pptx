# Milestone analysis scripts

Ad-hoc measurements behind the text models in `crates/pptx-ir/src/text.rs`
and the findings in `docs/phase0-findings.md`. They compare the PDFs of a
Windows milestone round (PowerPoint's export, `<name>-windows.pdf`) with
Typst's, using `text_positions.py`'s line matching. Run from the repository
root with the validator venv, e.g.

```sh
D=out/milestone/typst-pptx-phase3-b64304c
uv run -q --project tools/validate python -I tools/validate/analysis/spacing_fit.py \
  $D decks-features $D decks-text-metrics $D signal-wave-demo
```

| Script | Measures | Arguments |
|---|---|---|
| `spacing_fit.py` | per paragraph: exact spacing vs PowerPoint's pitch (whole-point rounding) and first baseline vs 0.75 × spacing | pairs: milestone dir, deck name |
| `list_lines.py` | per list line: spacing, space before, dy and step (space-before rounding) | milestone dir, deck names… |
| `list_model.py` | PowerPoint list model checked on the old IR, predicted for a new IR | milestone dir, new IR dir, deck names… |
| `predict_box_moves.py` | PowerPoint dy after moving text boxes to a new IR's position | milestone dir, new IR dir, deck names… |
| `line_widths.py` | line width export vs Typst (width check) | triples: IR, Typst PDF, exported PDF |
| `unmatched.py` | lines the matcher can't find, and why | IR, Typst PDF, exported PDF |
| `baseline_fit.py`, `baseline_pitch_fit.py` | Phase 0 first-baseline fits | triples: IR, Typst PDF, PowerPoint PDF |
| `embedded_fonts.py` | extracts a deck's embedded fonts, checks them with fontTools and OTS | pptx, output dir |
| `glyph_diff.py` | CFF→TrueType conversion: unhinted per-glyph pixel diff against the original | pairs: original, converted font |
| `style_font_probe.py` | one-slide deck, regular/bold/italic runs per embedded family (Plex and converted Libertinus, renamed), to see whether PowerPoint uses embedded non-regular faces; plain Python | template deck, Libertinus deck, output |
| `web_font_probe.py` | one-slide deck, one line per embedded-font variant (hinting, `post`, Mac names, PANOSE, size), to find what PowerPoint for the web rejects; plain Python | template deck, Libertinus deck, output |

The two font scripts take other dependencies than the validator venv; their
docstrings have the `uv run --with …` line.

Milestone dirs hold `decks/<name>.ir.json`, `decks/<name>.pdf` and
`<name>-windows.pdf` (see `tools/milestone.sh`). Known limit: text that
occurs several times on a slide can be matched to the wrong line.
