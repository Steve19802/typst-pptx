# Phase 0 findings

Facts checked against Typst 0.15.1 / Touying 0.8.0 that the plan depended on.
Reproduce with `typst-pptx dump --root . fixtures/themes/signal-wave/demo.typ --tags --paths`.

## Environment

- Touying 0.8.0 fails on `#figure[..]` without `caption:` under Typst 0.15.1
  (`field "caption" in figure is not known at this point`, touying#434/#445,
  fixed on main, unreleased). The signal-wave fixture passes `caption: none`.
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
| exactly S | 0.744 × S | S − 0.2 em |
| pitch | S (single: ≈ 1.2 em) | S (single: 1.2 em) |

- **LibreOffice**: `tools/validate/calibrate_baseline.py` (four fonts with
  very different metrics, 12–48 pt, single and 0.8–1.6 em exact), exact to
  0.001 em. Each line's baseline sits 0.2 × *that line's* largest size above
  its bottom, so lines with different sizes can't share an exact spacing.
- **PowerPoint**: fitted on the exporter's own output (the round-1 milestone
  decks exported to PDF on Windows, 291 single and 43 exact lines, residual
  ≤ 0.18 pt single, ≤ 0.67 pt exact). Mixed sizes in a paragraph are fine.
  The python-pptx calibration deck gave 0.935 em for single spacing in
  PowerPoint, so PowerPoint reacts to markup the exporter doesn't share
  (python-pptx template styles, `ea`/`cs` fonts, `kern`); calibrate with the
  exporter's output only.
- PowerPoint's exact-spacing pitch deviates from S by up to ±0.5 pt on some
  lines (e.g. 38.4 pt → 37.92 pt); not explained yet, within tolerance.
- Applying the PowerPoint model to the round-1 measurements predicts a
  vertical error of median 0.05 pt, max 0.67 pt (was 0.71 / 6.52 pt with the
  LibreOffice model). Horizontally PowerPoint is within 0.19 pt on every line.

`--text-model powerpoint` (default) or `libreoffice` picks the model; CI's
visual diff and text-position check use `libreoffice` (all lines within
0.1 pt), milestone checks verify `powerpoint`.

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
