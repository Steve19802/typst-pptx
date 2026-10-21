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
- Embedded: TrueType outlines, single fonts, `fsType` not restricted and not
  bitmap-only. CFF fonts (e.g. Typst's Libertinus Serif, used by the feature
  deck as fallback for a list marker) are reported, not embedded.
- Whole fonts, no subsetting: about 200 KB per face (demo 1.1 MB). Subsetting
  is a later size optimisation.
- LibreOffice (Debian build, no libeot) ignores EOT fonts, so CI checks the
  EOT structure in unit tests only; rendering is checked on Windows without
  the fonts installed.

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
