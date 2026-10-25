# Phase 0 findings

Facts checked against Typst 0.15.1 / Touying 0.8.0 that the plan depended on.
Reproduce with `typst-pptx dump --root . fixtures/themes/signal-wave/demo.typ --tags --paths`.

## Environment

- Touying 0.8.0 fails on `#figure[..]` without `caption:` under Typst 0.15.1
  (`field "caption" in figure is not known at this point`, touying#434/#445,
  fixed on main, unreleased). The signal-wave fixture passes `caption: none`.
- Touying 0.7.4 compiles under Typst 0.15.1 without errors, so an old deck
  would export silently with 0.7 pause semantics. The World records every
  package it loads; `@preview/touying` < 0.8 stops the export (exit 2, upgrade
  hint), > 0.8 warns (`fixtures/decks/errors/touying-0.7.typ`, tests/cli.rs).
- typst-kit 0.15.1's `SystemDownloader` is native-tls/OpenSSL only. The
  frontend uses its own rustls `Downloader` (ureq 2), so no OpenSSL is linked.
- `typst-pptx` diagnostics are byte-identical to `typst compile` for the
  signal-wave demo (four font warnings), colour codes aside.

## Tags (plan §4.4)

Elements with start/end tags in the frames of the signal-wave demo:
`par`, `item` (list/enum), `heading`, `equation` (inline and block), `table`,
`grid`, `cell` (grid and table cells share the name; compare the element, not
the name), `figure`, `image`, `place`, `raw` + `line` (raw lines), `strong`,
`outline` + `entry`, `link-marker`, `metadata`, `context`, `layout`,
`counter-update`, `artifact`, `pdf-marker-tag`.

Consequences:
- **Only real paragraphs get `par`.** Text placed directly in a `stack`,
  `block`, grid cell or `text(..)` call (card titles, slide header title, table
  cell text, footer) has no `par` tag. Paragraph grouping (§5.2) needs a
  fallback: consecutive text items under the same innermost element and frame,
  split by line position.
- **Page header/footer content sits inside `artifact` tags.** A direct signal
  for layout extraction (§7) and placeholder roles, besides repetition.
- Line frames: a line that contains inline math or boxes is its own group
  (one group per line); plain lines are flattened into the parent frame.

## Fonts (plan §5.1)

- Upstream IBM Plex TTFs (3.004/3.005) name Medium/SemiBold faces
  "IBM Plex Sans Medm"/"IBM Plex Sans SmBld" in name ID 1 (ID 16 is
  "IBM Plex Sans"). Typst groups faces by ID 1 minus known style suffixes, so
  `weight: "semibold"` renders with `IBMPlexSans-Bold` and `weight: "medium"`
  with `IBMPlexSans` (Regular). The exporter just follows what Typst picked;
  the legacy family name in ID 1 is also what PowerPoint addresses.
- Math uses Typst's embedded "New Computer Modern Math" (weight 450).

## Images (plan §5.5)

- `image(.., width: w, height: h)` defaults to `fit: "cover"`: the image is
  scaled to cover the box and wrapped in a clipping group. signal-wave's
  spectrum bars (60×12 box) are drawn as a 60×18 image clipped to 60×12; the
  tag icon (10×6 box) as a 100×6 image clipped to 10×6. The SVG's own
  `preserveAspectRatio="none"` never applies. The exporter must reproduce the
  crop (`a:srcRect`), and SVG→native conversion must apply the same clip.

## Baseline model (plan §5.2)

Both applications place text independently of the font's vertical metrics,
but with different constants (first baseline below the box top, zero insets):

| Spacing | PowerPoint 365 (2609) | LibreOffice 25.2 |
|---|---|---|
| single | 0.949 em | 1.0 em |
| exactly S | 0.75 × S | S − 0.2 em |
| pitch | S (single: ≈ 1.2 em) | S (single: 1.2 em) |

- **LibreOffice**: `tools/validate/calibrate_baseline.py` (four fonts with
  very different metrics, 12–48 pt, single and 0.8–1.6 em exact), exact to
  0.001 em. Each line's baseline sits 0.2 × *that line's* largest size above
  its bottom, so lines with different sizes can't share an exact spacing.
- **PowerPoint**: fitted on the exporter's own output, two milestone rounds
  exported to PDF on Windows. Round 1 (built with the LibreOffice model):
  291 single lines give 0.949 em (residual ≤ 0.18 pt). Exact spacing over
  both rounds (48 paragraphs, S = 0.34–3.7 em, 10–44 pt): 0.75 × S, residual
  median 0.10 pt, 90 % ≤ 0.20 pt. Mixed sizes in a paragraph are fine.
  The python-pptx calibration deck gave 0.935 em for single spacing in
  PowerPoint, so PowerPoint reacts to markup the exporter doesn't share
  (python-pptx template styles, `ea`/`cs` fonts, `kern`); calibrate with the
  exporter's output only.
- Open: one paragraph (36 pt, S = 1.2 em, text-metrics slide 3) starts
  1.5 pt lower than the model while 24 pt at the same ratio fits; and
  PowerPoint's exact pitch deviates from S by −0.4…+0.5 pt on some
  paragraphs. Both within the Phase 1 tolerance; more data from later
  milestones.
- Round 2 (exporter with the PowerPoint model, 0.744 × S at the time),
  measured: demo 286 lines |dy| median 0.05 / max 0.68 pt, feature deck
  0.08 / 0.58, touying-basic 0.09 / 0.16, text-metrics 0.30 / 1.72 (the
  sizes and spacings that led to 0.75). Horizontally ≤ 0.19 pt everywhere.
  All manual checks passed (no repair prompt, SmBld font name, radius
  handle, alt text, editing without re-wrap, notes).

`--text-model powerpoint` (default) or `libreoffice` picks the model; CI's
visual diff and text-position check use `libreoffice` (all lines within
0.1 pt), milestone checks verify `powerpoint`.

## Font embedding (plan §5.1, Phase 1)

- `.fntdata` parts are Embedded OpenType, not Word-style obfuscated fonts.
  The exporter writes EOT 0x00020001 with the TrueType data uncompressed and
  unencrypted, one part per face, listed per family (regular/bold/italic/
  boldItalic) in `p:embeddedFontLst` with the run's legacy family name.
- Embedded: single fonts, `fsType` not restricted and not bitmap-only. CFF
  fonts are converted to TrueType first (below; until 0.1.0 they were
  reported, not embedded).
- Whole fonts, no subsetting: about 200 KB per face (demo 1.1 MB). Subsetting
  is a later size optimisation.
- LibreOffice (Debian build, no libeot) ignores EOT fonts, so CI checks the
  EOT structure in unit tests only; rendering is checked on Windows without
  the fonts installed.

## CFF fonts converted to TrueType (0.1.1)

- Found on tino (Ubuntu, 2026-10-22): without the theme's fonts, Typst
  falls back to its built-in Libertinus Serif (CFF), and PowerPoint reported
  "Libertinus Serif" and "Libertinus Serif Semibold" missing. All of Typst's
  built-in text fonts except DejaVu Sans Mono are CFF (Libertinus Serif, New
  Computer Modern), so any deck on default fonts hit this. Even with IBM
  Plex, the feature deck's nested-list marker `‣` comes from Libertinus.
- `typst-frontend/src/truetype.rs`: cubics → quadratics within 1 font unit
  (piece count from the √3/36 third-difference bound, no iteration),
  contours reversed to clockwise, `maxp` 1.0, long `loca`, no hinting.
- `hmtx` left side bearings must be rewritten to the new `xMin`: TrueType
  rasterisers place the outline by the bearing, and in the CFF originals
  469 of 2793 (Regular) differ, up to 23 units (Italic `@`), which shifted
  glyphs sideways. `hhea` extents and `head` bbox are recomputed too.
- Checks: OTS and a full fontTools glyph decode accept all faces;
  unhinted FreeType renders at 200 px differ from the originals by
  0.004–0.010 % of ink over ~2,100–2,400 characters per face (hinted: 6–9 %,
  FreeType grid-fits CFF stems only). Scripts:
  `tools/validate/analysis/embedded_fonts.py`, `glyph_diff.py`. LibreOffice
  ignores embedded fonts, so the look in PowerPoint is a Windows check.
- Sizes: Libertinus ~0.6 MB, New Computer Modern ~1 MB per face; the
  feature deck embeds Libertinus Regular (650 KB) for one bullet glyph.
  Subsetting is the follow-up.
- Lists in Libertinus fell back to text: the pitch (e.g. 22.68 pt) rounds
  up to 23 pt and the gaps can't absorb the negative remainder (space
  before is ≥ 0). Lists now retry with the spacing rounded down (as table
  cells do); with IBM Plex the output is unchanged (pitch already whole).
  Demo without IBM Plex: 9 of 9 lists native (was 1).
- PowerPoint for the web (2026-10-24): the slide thumbnails (rendered on
  the server) use the converted Libertinus, but the editing canvas
  substitutes a sans font. IBM Plex looked fine there only because it is a
  Microsoft cloud font: in a probe deck
  (`tools/validate/analysis/web_font_probe.py`), the original IBM Plex Mono
  displayed, and the same font renamed did not, nor did any variant (no
  hinting, `post` v3, no Mac records, zero PANOSE, Libertinus `head` flags,
  all together) or converted Libertinus (as exported, renamed, cut to 73
  KiB). The web canvas ignores embedded fonts altogether; it only has
  installed and cloud fonts. Nothing to fix in the converter; the desktop
  app remains the check.
- Same probe deck in the PowerPoint desktop app (Windows, 2026-10-24): all
  Plex Mono variants (P0–PG) drawn in the embedded font, so renaming,
  missing hinting, `post` v3, no Mac records, zero PANOSE and the `head`
  flags are all accepted. The converted Libertinus lines (L0 as exported,
  L1 renamed, L2 cut to ASCII) are drawn in Libertinus: PowerPoint accepts
  the CFF→TrueType conversion (screenshot in
  `out/milestone/fonts-0.1.1/web-font-probe-windows.png`).
- **Resolved (2026-10-24), see the last items:** the real decks exported to PDF from the desktop app
  (`out/milestone/fonts-0.1.1/signal-wave-demo-libertinus.pdf`,
  `new-computer-modern.pdf`) draw bold and semibold text in the *regular*
  face: each PDF has one face per family (the IBM Plex demo has three Plex
  Sans faces). Italic looks slanted. Line positions are fine (216 lines,
  |dy| median 0.07 pt, max 0.73 pt). Style flags, weights, names and EOT
  headers match IBM Plex's, but IBM Plex is a cloud font, so embedded
  non-regular faces may never have been exercised. Probe:
  `tools/validate/analysis/style_font_probe.py` → `style-font-probe.pptx`.
- Style probe in the desktop app: dialog "Install Embedded Fonts: Some of
  the embedded fonts in your presentation cannot be installed", Font
  Installation Failed for Probe S0/S1/S3/S4 **Bold**, i.e. also for the
  original IBM Plex Sans Bold under a new name. Italic faces install. So
  embedded bold faces fail in general (not the CFF conversion), hidden so
  far because IBM Plex is a cloud font; `pitchFamily`/`charset` on
  `p:font` don't help. Next: a reference deck where PowerPoint embeds the
  same faces itself (`out/milestone/fonts-0.1.1/reference-fonts/`, Plex
  renamed "Probe Ref"), to compare its `.fntdata` with ours.
- But the probe's PDF export (`style-font-probe.pdf`) contradicts the
  dialog: in the renamed families regular **and bold** are the embedded
  faces, italic falls back to Arial. Cause of the italic: the probe's own
  EOT writer set Italic = 0 (fixed in `web_font_probe.py`), so its italic
  face posed as a second regular. S2 ("Libertinus Serif") rendered exactly
  as the demo opened earlier: PowerPoint keeps embedded families per
  session by name, so probes need a fresh PowerPoint.
- Difference left between the probe (bold works) and typst-pptx decks (bold
  drawn regular in the demo and in `new-computer-modern.pdf`, where italic
  and bold italic work): typst-pptx writes face parts in first-use order, so
  the **bold part comes before the regular one** (`font1`, lower `rId`).
  Probe: `tools/validate/analysis/order_font_probe.py` →
  `order-font-probe.pptx`.
- Order probe, fresh PowerPoint, no dialog (`order-font-probe.pdf`): all
  five families draw real regular, bold, italic and bold italic faces,
  bold-first included. **Part order is not the cause**; embedded
  non-regular faces work, for original and converted fonts alike.
- Zoomed `new-computer-modern.pdf`: bold runs are the regular face without
  synthetic bold, "both" is italic not bold; italic is the real face. Left
  between probes and real decks: the fonts' own names (probes renamed them;
  NCM has typographic names 16/17) and session state (the real decks were
  opened after `web-font-probe.pptx`, whose "Libertinus Serif" has only a
  regular face). Probe: `name_font_probe.py` → `name-font-probe.pptx`.
- Name probe, fresh PowerPoint (`name-font-probe.pdf`): NCM and converted
  Libertinus under their own family names lose bold (bold → regular, bold
  italic → italic); the same faces renamed work. In the PDF, N1's four runs
  share one font (named after NCM's typographic family), N2's are four.
  The fresh demo export (`signal-wave-demo-libertinus-fresh.pdf`) is
  identical to the first. Libertinus's name tables differ from the renamed
  copies only in the values of IDs 1, 3, 4, 6, so the **family name
  itself** is known to the machine: installed fonts, or PowerPoint keeping
  embedded families across restarts (the first "Libertinus Serif" opened,
  in `web-font-probe.pptx`, had the regular face only). Probe:
  `persist_font_probe.py` → `persist-a.pptx`, `persist-b.pptx`.
- Cause found: the test machine had **installed** the original CFF
  `LibertinusSerif-Regular.otf` and `NewCM10-Regular.otf`, regular faces
  only, per user (`%LOCALAPPDATA%\Microsoft\Windows\Fonts`, Windows'
  "Install for me" folder, not PowerPoint's). An installed family takes
  precedence over the embedded one, so bold had no face. Not an exporter
  bug; the persistence probe is moot. PowerPoint does not install embedded
  fonts (they would be `.ttf`, our converted copies; these are the CFF
  originals under Typst's file names). Users with part of a family
  installed will see the same.
- Confirmed after uninstalling them (`signal-wave-demo-libertinus-fresh2.pdf`):
  the demo's PDF has three embedded Libertinus faces (regular, semibold,
  bold weights), title bold, card headings semibold. Typst vs PowerPoint:
  SSIM 0.916–0.988 per slide (was 0.841–0.966); 216 lines, |dy| median
  0.07 max 0.73 pt, |dw| median 0.22 max 3.36 pt (was 25.4, bold drawn
  regular). The CFF→TrueType embedding (0.1.1) is confirmed in the
  PowerPoint desktop app for regular, semibold, bold and italic faces.

## Paragraph grouping (plan §5.2)

- Typst keeps the space before a soft line break inside the line's text item
  ("wraps over "). The exporter trims it: PowerPoint ignores trailing spaces
  for alignment.
- Typst centres and right-aligns lines including that trailing space and
  with `text.overhang` (on by default): line-final punctuation hangs into the
  margin (a final "." sits 3.9 pt past the right edge at 18 pt). PowerPoint
  can reproduce neither, so centred and right-aligned paragraphs whose edges
  don't line up within 0.5 pt stay one text box per line (exact look).
  Left-aligned and justified paragraphs group into one box.

## Line widths and justification (plan §5.2, Phase 1)

- Measured line widths (first to last character) against Typst: PowerPoint
  (round 2) median 0.11–0.19 pt, max 1.29 pt (0.5 %); LibreOffice median
  0.05–0.27 pt, max 1.56 pt. Both kern with IBM Plex's GPOS `kern` (it has
  no legacy `kern` table), so Typst's advances carry over.
- Justified lines: Typst stretches the space glyphs. Spaces become their own
  runs with `spc` = tracking + stretch; LibreOffice then ends justified
  lines within 0.16 pt.
- Width check: per cluster, the font's advance of the characters looked up
  one by one vs. Typst's glyphs (ligatures, other shaping). A line off by
  more than max(1 pt, 0.5 %) sends its paragraph to line mode, counted in
  the report. No fixture line triggers it.
- `text_positions.py` reports width differences (dw); CI fails above 2 pt.

## Baked fallback (plan §5.8, Phase 1)

Items the mapping can't express are baked into a picture at their exact
position instead of being dropped, and counted by reason in the report:
rotated/scaled/skewed text and images, gradient or tiling text, shapes with
unsupported paint or transforms; equations as before
(`fixtures/decks/fallbacks.typ`). Baked SVGs that use `<pattern>` (Typst's
tilings and conic gradients) are PNG only, like images.

Known: a tiling's phase can shift by part of a cell, because the leaf is
baked without its parent container. pdfium (used by the visual diff) draws
Typst's gradient text in a flat colour; the baked picture is right.

## Gradients and strokes (plan §5.4, Phase 1)

- Linear gradients relative to the shape → `a:lin scaled="1"` with Typst's
  aspect-corrected angle (both describe the gradient on the shape's unit
  square, corner to corner). Centred radial gradients → `a:path circle`,
  stop positions scaled by r/√0.5 (DrawingML's circle reaches the corners).
  Typst interpolates in Oklab by default, so 17 stops are sampled from the
  gradient instead of copying its stops. Parent-relative, conic and
  off-centre radial gradients are baked.
- Strokes: `a:custDash` in thousandths of a percent of the width, caps and
  joins incl. miter limit; a non-zero dash phase is dropped (fidelity
  normalized).
- Open: with round caps, PDF/Typst extend each dash by the caps into the gap
  (Typst's 3 pt `dotted` = dash 3, gap 2 → nearly touching dots);
  LibreOffice draws the dash length including the caps. PowerPoint's
  behaviour decides whether to compensate (features slide 5 at the
  milestone).

## Phase 3 milestone result (b64304c, PowerPoint desktop)

- Tables: no row growth (borders and following text where Typst has
  them); cell text within 0.33 pt (tables deck), demo table slides within
  0.3 pt. PowerPoint places cell text like text boxes (the text-box model
  holds in cells). Links in cells are underlined (run-level only).
- Lists: PowerPoint rounds space before to whole points too (0.22 and
  0.44 pt drawn as 0), so lists drifted ~0.22 pt per item (demo slide 13:
  −1.27 pt at the end). Now space before is written in whole points with
  the rounding carried to the next item, and the box is placed so the
  remaining error is split between first and last line: predicted max
  1.09 → 0.52 pt (model vs measured: median 0.09 pt).
- Manual checks passed: demo slide 10 one table, 3.06" × 10.58" (Typst:
  7.77 × 26.88 cm), tables deck 1.18" high on both slides; merged cells,
  typing in a cell without row growth, list Enter/numbering/levels, demo
  lists. On demo slide 10 PowerPoint shows "a very narrow empty column"
  on the right while the table is selected; the cursor can't enter it
  (it moves on to the next row), the grid has 6 columns summing to the
  frame width and the PDF shows nothing: a selection display, not part of
  the table. Phase 3 is complete.
- Measurement note: text that occurs several times on a slide ("0.0 dB")
  can be matched to the wrong line by `text_positions.py` (+65 pt).

## Native bullets (Phase 3)

- Typst 0.15 tags each list item's marker (`pdf-marker-tag`, kind
  `ListItemLabel`) and body (`ListItemBody`); `item` tags are emitted out
  of place and aren't usable. All text of an outermost `list`/`enum` forms
  one text box: one paragraph per item with `marL` at the body and a
  hanging `indent` to the marker, `lvl` for nesting; `buChar` with the
  marker's font, size and colour, or `buAutoNum` (arabic/alpha with
  period, `)` or both parentheses; `startAt` when a run doesn't start at
  1). An item's further paragraphs get `buNone`.
- Pitch: the smallest gap between lines; larger gaps and the
  application's spacing rounding become `spcBef` (hundredths of a point;
  whether PowerPoint rounds it is a Phase 3 milestone check).
- Plain text instead (reported): multi-character bullets, unknown
  numbering, numbers that don't count up, markers off their body's first
  line, mixed sizes under the LibreOffice model, `--text flow`. No fixture
  list falls back; features: 2 list boxes instead of 10 boxes, demo: 9.

## Native tables (plan §5.6, Phase 3)

- Typst gives no frame per cell: cell tags sit at the cell's top-left corner
  and carry the resolved `x`/`y` (spans unset = 1); the table's frame group
  closes the last column and row. Fills are one rectangle per cell, borders
  full-length lines extended by half their width at both ends.
- Mapping: column/row lines from the cell tags; fills matched to cells;
  border lines split into grid segments, one line per cell side (a border
  through a merged cell or over part of a side → shapes). Cell text goes
  through the text-box pipeline; margins put it where Typst did and give it
  the widest room its alignment allows (left: to the right edge, centred:
  symmetric), so PowerPoint has no reason to wrap. Empty cells get a 1 pt
  end mark.
- Row growth: text must fit its row (top margin + lines × spacing). Where
  the model's spacing doesn't fit (Typst's default inset 5 pt, 16 pt, two
  lines: 2 × 22 > 42.75), spacing is rounded down to whole points; text
  up to 0.5 pt above the cell starts at its top.
- LibreOffice lays out cell text with the font's hhea metrics (baseline at
  the ascent; exact spacing: descent above the line bottom), unlike its text
  boxes (1.2 em / 0.2 em). Modelled for the CI check: cell lines within
  0.14 pt (demo) and 0.37 pt (tables deck). PowerPoint's cell model is
  unmeasured: same as text boxes until the Phase 3 milestone.
- Shapes instead (reported with the reason): equations, images or tables in
  cells, gutters, more than one text block in a cell, rotated tables,
  diagonal or off-grid lines. Demo: slides 9 and 10 native, slide 14 (an
  equation in a cell) shapes. `fixtures/decks/tables.typ` covers default
  strokes, alignment, wrapping, links, merged cells, partial strokes and
  the fallbacks.

## Phase 1 milestone result (1d819c8, PowerPoint desktop, fonts embedded)

IBM Plex not installed; the embedded fonts render and stay editable. Text
positions against Typst (Print to PDF):

| deck | lines | dy median | dy max | dx max |
|---|---|---|---|---|
| signal-wave-demo | 265 | 0.06 | 0.46 | 0.35 |
| features | 32 | 0.09 | 0.51 | 0.80 |
| text-metrics | 44 | 0.08 | 1.61 (36 pt outlier) | 0.02 |
| touying-basic | 24 | 0.09 | 0.16 | 0.05 |

The whole-point spacing model matched its prediction (features 1.01 →
0.51, demo 0.80 → 0.46). Outline page numbers within 0.45 pt (the 2 pt in
LibreOffice is its own rounding). Demo slide 2: no underline, rows jump to
their slides. Rotated text matches visually (pdfium can't extract it from
Print to PDF, so it isn't measured).

Manual checklist (3.1–3.12): all passed, including fonts after Save As,
the badge SVG as editable shapes, links in the slide show, typing in the
rotated paragraph and the fallback deck. Phase 1 is complete.

## Gaps within a line, links on boxes (Phase 1 milestone)

- The signal-wave outline was ~230 text boxes: every leader dot of
  `repeat` was its own box (pieces joined only when touching), and the dots
  sat in an inline `box`, so they had another paragraph key. Now pieces of
  one key on one baseline are joined: gaps at least a space wide become a
  space with extra spacing, narrower ones extra spacing on the character
  before (the dots merge into one run); rounding to hundredths of a point
  is carried to the next gap. Loose text is keyed by its innermost
  non-inline element (link, repeat, box, strong, … skipped). Outline: 4
  boxes, one per row; sentences with inline equations are one line.
- LibreOffice puts the outline's page numbers 0.6–2 pt right of Typst
  (~0.025 pt per dot, its 1/100 mm character grid); check in PowerPoint.
- PowerPoint underlines linked runs regardless of `u="none"` (milestone 1,
  demo slide 2). A link shared by all of a box's text is set on the box
  (`p:cNvPr/a:hlinkClick`): no underline, the box is clickable. Links on
  part of a text stay on the runs and are underlined by PowerPoint.

## PowerPoint rounds exact line spacing (Phase 1 milestone)

- Measured pitch over 22 uniform paragraphs in both milestone rounds:
  PowerPoint draws exact spacing rounded to whole points (24.26 → 24.00,
  23.77 → 24.00, 20.22 → 20.00; residual ≤ 0.05 pt, a few 0.1 pt). The
  first baseline is 0.75 × the rounded spacing. The same in Phase 0
  (fonts installed) and Phase 1 (embedded), so not a font effect.
- The PowerPoint model writes the rounded spacing (what it draws anyway)
  and places the box so the paragraph's middle line is exact; the pitch
  error is split between first and last line. Predicted from the Phase 1
  PDFs: features max |dy| 1.01 → 0.57 pt, demo 0.80 → 0.42 pt,
  text-metrics p90 0.67 → 0.26 pt (max stays the 36 pt outlier).
- Phase 1 round 1 PDFs were browser "Save as PDF" (page rounded to 96 dpi
  pixels, every line shifted); `text_positions.py` now warns about those.

## Hanging punctuation (Phase 1 milestone)

- Typst hangs line-final punctuation past the right edge (`overhang`, on by
  default: `.` `,` 80 % of the advance, `-` 55 %, `:` `;` 30 %, dashes
  20 %). Centred and right-aligned paragraphs whose last line ended in "."
  failed the edge check and came out as one box per line (milestone 1,
  features slide 2).
- The edge check also accepts edges without the overhang. PowerPoint
  doesn't hang, so such paragraphs become one paragraph per line inside a
  single text box, each placed by `marL`/`marR`: exact in LibreOffice
  (within 0.04 pt). Negative spacing on the hanging glyph was tried first:
  LibreOffice ignores spacing after a line's last character when aligning.
- LibreOffice carries an omitted `marL`/`marR` over from the previous
  paragraph, so these paragraphs always write both.

## Rotation (plan §5.7, Phase 1)

- Transforms that only rotate and translate are mapped natively: the item
  is laid out in its unrotated frame (text lines, shape geometry, flips,
  gradients) and the frame turned about its centre (`a:xfrm rot`).
  DrawingML flips before rotating and rotates gradients with the shape
  (`rotWithShape="1"`), as Typst does. Text at different angles never
  shares a paragraph. Features slide 9: SSIM 0.993, nothing baked.
- Each item is rotated on its own; a rotated Typst group is not a rotated
  `p:grpSp` (same look, the parts move separately when edited).
- Still baked: scale, skew and mirroring; rotated images cut by a clip.
  Rotated SVG images stay pictures (no native shapes).

## Hyperlinks (plan §5.7, Phase 1)

- Typst link areas (`FrameItem::Link`) are collected per page; a text item
  gets the link whose area holds its middle, a picture the one at its
  centre. Typst splits text items at link boundaries, so runs split there
  too.
- URLs → external `hyperlink` relationships; links to labels or positions →
  `ppaction://hlinksldjump` to the slide showing that page. With
  `--animations none`, a dropped subslide's page links to its logical
  slide.
- Linked runs carry `u="none"` (LibreOffice, and PowerPoint by default,
  underline hyperlinks; Typst draws underlines as separate lines) and the
  Office 2019+ `ahyp:hlinkClr val="tx"` extension so the run keeps its own
  colour. Older PowerPoint uses the theme's hyperlink colour: check at the
  milestone (features slide 8, demo outline slide 2).

## SVG as native shapes (plan §5.5, Phase 1)

- Trees of groups and paths with solid paint and ≤ 60 nodes become a group
  of native shapes (`p:grpSp`), through the same curve recogniser as Typst
  shapes (rect/ellipse/roundRect/custom). Typst scales SVGs by
  size / tree size, the conversion does the same.
- SVG element `opacity` arrives as group opacity and is folded into the
  paint alpha (exact for one path with only fill or only stroke, else
  fidelity approximate).
- Clipped SVGs (`fit: "cover"`): shapes outside the visible part are
  dropped, filled rectangles crossing it are cut; anything else crossing it
  keeps the picture. signal-wave: 22 of 69 SVG uses are native (sine waves,
  noise, dividers); spectrum bars (rounded rects crossing the clip), tag
  icons and waveform frames stay pictures. General path clipping would
  convert those.
- `--svg auto|picture`, `--svg-max-nodes 60`.

## SVG in PowerPoint (plan §5.5)

signal-wave's background grid is an SVG built from `<pattern>` fills.
LibreOffice renders `asvg:svgBlip` itself and draws patterns visibly heavier;
Office's SVG renderer is not reliable with patterns either. SVGs that use
`<pattern>`, `<filter>`, `<mask>` or `<foreignObject>` are exported as PNG
only (2 px/pt); simple SVGs as SVG with a PNG fallback.

## Touying metadata (plan §6.1)

The `<pdfpc-file>` metadata value is
`{pdfpcFormat: 2, disableMarkdown, pages: [{idx, label, overlay,
forcedOverlay, hidden, note?}]}`; `note` is plain text, paragraphs separated
by a blank line. The adapter reads it through the introspector; its shape is
frozen in `crates/touying-adapter/tests/pdfpc_shape.rs` (unknown page fields
are an error). A slide's notes may sit on any of its subslides (touying-basic:
on the last), so they are collected across the logical slide.

## Click animations (plan §6.2–6.3, Phase 4)

- Fixture `fixtures/decks/animations.typ`: pause (paragraphs, list, shapes),
  meanwhile, uncover/only, same-size alternatives, a layout shift, the alpha
  cover. 23 pages → 12 slides with `auto`.
- Touying's `hide` keeps the layout, so content that stays doesn't move:
  items are matched across subslides by their content key (the IR item
  without its name, frame included). Positional names ("Text 3") can't be
  used, they are numbered per page.
- List boxes are fitted to their visible lines, so a list revealed with
  `#pause` changes frame (y 1297496 → 1273048 → 1285113 EMU) and line
  spacing (none with one item). Paragraphs carry `anchor` (Typst's first
  baseline, not dumped) and are compared by runs, level, bullet and anchor
  only; a box whose paragraphs are a subset/superset of an earlier version
  is the same box, revealed by paragraph (`p:txEl`/`p:pRg`, `bldP
  build="p"`). The slide shows the version with all paragraphs.
- Alpha cover (`alpha-changing-cover`): text alpha 0x40 → 0xff, nothing
  else changes. The dimmed and full versions are separate items swapped on
  the click (Disappear + Appear), exact and simpler than emphasis effects.
- Layout shift = in one step the same content (key without frame) leaves
  one place and enters another, or a list paragraph's anchor moves. The
  logical slide is split there; the next slide gets Morph
  (`mc:AlternateContent`, `p159:morph`, fade fallback) and items showing
  the same content across the boundary share `!!Morph N` names.
- Merged slides renumber positional names (unique per slide); report
  entries of an item on several subslides of one slide are counted once
  (per-page counts kept: the demo still reports 29 baked equations).
- Validation: `NAME.pptx` (auto) passes the Open XML SDK; the LibreOffice
  comparison and the milestone PDF export use `--animations slides`
  (PowerPoint's PDF export draws a slide once with all its objects).
- Milestone `phase4-d7a7420`, PowerPoint 365 desktop (owner, 2026-10-24):
  the animations deck plays as intended; by-paragraph list builds behave
  like Typst's subslides; Morph at `spd="fast"` is fine. Measured PDF
  exports (`--animations slides` copies): animations deck 115 lines, |dy|
  median 0.09 max 0.37 pt, |dw| max 1.27 pt, SSIM ≥ 0.977; every other
  deck's minimum SSIM and line offsets identical to Phase 3 (outliers: the
  known 36 pt paragraph, 1.61 pt, and the "0.0 dB" mismatch, 65 pt).
  PowerPoint 2021 not tested.

## Slide layouts (plan §7, Phase 5)

- signal-wave demo: 16 items repeat (same content key, frame included) on
  2–10 slides. Content slides share 3 background items (bottom of the
  z-order) and 4 footer items (near the top); section slides 6 items,
  title/ending 5. Identical pictures already share one media part, so the
  saving is XML only (1.057 → 1.039 MB); the point is editing (one place
  for the theme's chrome).
- The decorations are `place`d in the slide body, so Typst draws them
  **above** the header title (header before body). A layout is always
  below the slide's items: an item moves only if no lower item that stays
  on the slide overlaps it, **except text boxes** (faint decorations over a
  title; the theme's intent is a background). Overlapping shapes or
  pictures keep the item on the slide.
- Repetition counts **logical slides**: with `--animations slides` (and
  Morph splits) subslides share their content, which must stay on the
  slides (first try moved "First paragraph" of a pause slide to a layout).
- Slides are grouped by the exact set of movable items; a set on ≥ 2 slides
  is a layout, a slide with a unique set takes the largest layout it fully
  shows. A background colour shared by all slides of a layout moves to it.
  Animated items stay on the slide; click targets are renumbered.
- Demo: 3 layouts (content ×10, section ×3, title/ending ×2), outline and
  focus slides blank. LibreOffice renders layout content: SSIM and text
  positions unchanged (`text_positions.py` now includes layout text).
- `--layouts auto|none` (default auto).
- Known limit: identical text on two different logical slides (e.g. a
  repeated "Questions?" slide) would go to a layout too.

## Theme colours and fonts (plan §7, Phase 5)

- Touying keeps `config-colors` in its own state (no metadata), so the
  palette is read back from the paints: lt1 = most common slide background,
  dk1/dk2 = the two most used dark text colours (luminance < 0.2), lt2 =
  most used other light colour, accent1–6 = colours with chroma
  ((max − min)/255) ≥ 0.25; HSL saturation would rate pale greys such as
  `D8E2F0` as saturated.
- Accents used as fills or lines rank before text-only colours: the first
  try put the demo's syntax-highlighting reds (`D73948`, `B60157`) at
  accent2/4. Now the demo gives primary, secondary, tertiary as
  accent1–3 and dk1/lt1/dk2/lt2 = text-body, bg, text-dark, card-bg,
  matching `palette` in `template.typ`.
- Fonts: minor = family with most characters, major = same among text ≥
  24 pt. New text boxes in PowerPoint get IBM Plex Sans instead of Arial.
- Paints equal to a scheme colour (opaque part) are written as
  `a:schemeClr` (tx1/bg1/tx2/bg2/accentN/hlink, alpha kept), so Design →
  Variants recolours the deck. LibreOffice resolves them: SSIM and text
  positions unchanged.

## Placeholders from `pptx-hints` (plan §7, Phase 5)

- Package `packages/pptx-hints` (`@local/pptx-hints:0.1.0`; fixtures reach
  it through `fixtures/packages/local/`, a symlink). `role(kind, body)`
  wraps the body in `#metadata((kind: ..)) <pptx-role-start>` …
  `#metadata(..) <pptx-role-end>`: plain metadata is layout-neutral (a
  labelled box can't break lines, a labelled block adds spacing); the
  walker gives text pieces the role open at their position. The patched
  signal-wave renders byte-identical PNGs (17 pages) with and without it.
- Text with different roles never shares a box: signal-wave's title-slide
  `stack` puts title, subtitle and author under one grouping key.
- Role boxes are `p:sp` placeholders (`title`, `subTitle` idx 1, body idx 2,
  `dt` 10, `ftr` 11, `sldNum` 12, PowerPoint's own ids) with their own
  frame and explicit text; one per role and slide. They never move to a
  layout; each layout gets empty placeholders for its slides' roles.
- Slide number: the leading digits become `a:fld type="slidenum"` only if
  every slide-number box starts with its slide's position (otherwise
  Touying's numbers, e.g. after a Morph split, stay static). signal-wave's
  "n / N" → field + " / 17".
- LibreOffice imports placeholders as its own outline/title objects and
  **wraps them at the frame** whatever `wrap="none"` (on the slide or the
  layout) says: titles and footers fitted to Typst's widths broke ("From"
  on its own line, SSIM 0.90). Role boxes are widened towards the slide
  edges around their alignment point (left → right edge, centre →
  symmetric, right → left edge); unwrapped text doesn't move. Demo back to
  SSIM ≥ 0.952, 265 lines within 0.14 pt.
- Milestone `phase5-259557f` (owner, 2026-10-24): layouts, title/subtitle
  placeholders, slide-number field (reordering renumbers), theme colours
  (recolouring via Variants) and theme fonts pass; animations unchanged.
  Failed: every layout showed "Click to edit the outline text", and
  Insert → Header & Footer didn't know the footer. Cause: the master had
  no placeholders; layout placeholders inherit from the master's by type
  and the dialog works through that chain. The master now has PowerPoint's
  five (title, body idx 1, dt 2, ftr 3, sldNum 4) at the deck's role
  frames, else Office's proportions.
- Same round: Layout 3 showed a static "17". Touying's "n / N" is two
  elements (two boxes); the second lost its role (one per slide), was
  identical on every slide and moved to the layout. All text of a role on
  a page is now one paragraph group (`TextKey::Role`): one placeholder
  "4 / 17", field on the 4. PowerPoint has no total-slides field, so the
  total stays static text in each slide's placeholder.
- Re-check `phase5-1dd4b44` (owner, 2026-10-24): layout/master footer and
  slide-number placeholders had no format (Header & Footer inserted 18 pt
  black text), and the slides' footer/number looked like plain text boxes
  (Selection Pane: "Text 38"). The package links were right (same `idx`
  on slide and layout; LibreOffice imports them as footer/page-number).
  Now: empty master/layout placeholders carry the role's alignment and
  first run (font, size, bold, colour) as `a:lstStyle` level 1, and role
  boxes are named like PowerPoint's ("Title 1", "Footer Placeholder 51").
- signal-wave's section slides mark their heading `pptx.role("title")`:
  the section layout gets a title placeholder.
- Re-check `phase5-a346420`: LibreOffice's master view still showed the
  footer centred, 14 pt, uncoloured. LibreOffice ignores a footer/number
  placeholder's list style and formats its field from the paragraph's
  first run (centred 14 pt when empty). The owner fixed it in LibreOffice
  and saved as .pptx (`signal-wave-demo-footer-fixed.pptx` in that
  milestone folder), which shows both halves of the fix:
  - layout placeholders: `a:pPr algn` plus a run with full properties
    (`<footer>`, the number field);
  - slide placeholders: `<p:spPr/>` and runs without font, size or colour:
    everything inherited from the layout placeholder.
  Now master/layout placeholders hold the deck's footer text (or the
  number field) formatted explicitly, besides the list style; slide role
  boxes write no position where their frame equals the layout
  placeholder's, and runs in the placeholder's font, size and colour carry
  only language, spacing and link. Typst rendering in LibreOffice
  unchanged (SSIM, 265 lines within 0.14 pt).
- Re-check `phase5-e1b68fd` (LibreOffice): master formatted correctly,
  but slide 4's footer and number were still shapes beside the master's
  (Insert → Header & Footer added the real ones). LibreOffice imports a
  slide `ftr`/`dt`/`sldNum` placeholder as its page setting (master frame
  shown, `presentation:use-footer-name`) only when the placeholder has no
  shape, body, paragraph or run properties at all. Role boxes equal to
  their layout placeholder (frame, alignment, run format, no spacing or
  links) are now written bare (`<a:bodyPr/>`, `<a:p><a:r><a:t>`): the
  footer imports as LibreOffice's real footer. The slide number still
  imports as a shape because of the static " / 17" (tested: without it,
  `display-page-number="true"` and no shape).

## Milestone procedure

PowerPoint's own PDF export is the reference at milestones (owner's idea):
`make milestone` packages decks with Typst PDFs and IR, the owner exports
each deck to `NAME-windows.pdf`, `make milestone-report DIR=..` runs the
visual diff and the text-position check. PowerPoint prints 16:9 slides
centred on A4 (60.9 pt top offset); the tools crop/offset for that.

## Visual diff baseline

`make validate`, LibreOffice 25.2 vs Typst, 60 dpi: feature deck ≥ 0.995,
touying-basic ≥ 0.999, signal-wave demo ≥ 0.954 SSIM per page (CI threshold
0.94). The remaining demo differences are small horizontal drifts inside
lines (glyph advances/kerning), the Phase 1 width check.
