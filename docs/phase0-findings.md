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

Measured with `tools/validate/calibrate_baseline.py`: python-pptx boxes with
zero insets, no wrap, no autofit, two lines each, rendered by LibreOffice
25.2. Four fonts with very different vertical metrics (IBM Plex Sans,
Libertinus Serif, New Computer Modern, DejaVu Sans Mono; typo ascenders
0.76–0.89 em, hhea/win ascenders 0.89–1.13 em, `USE_TYPO_METRICS` on and off),
sizes 12/24/48 pt, single and exact 0.8–1.6 em spacing.

The result is **independent of the font's metrics**, within 0.001 em:

| Spacing | First baseline below box top | Line pitch |
|---|---|---|
| single | 1.0 em | 1.2 em |
| exactly S | S − 0.2 em | S |

This is the commonly described PowerPoint model (single line = 1.2 × size)
and explains the 0.22 em offset of the first line-mode export (1.0 − Plex's
0.78 em typo ascender). Implemented in `pptx-ir/src/text.rs`; the guide lines
in `calibration.pptx` check it in PowerPoint at the Windows milestone.

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

## Visual diff baseline

`make validate`, LibreOffice 25.2 vs Typst, 60 dpi: feature deck ≥ 0.995,
touying-basic ≥ 0.999, signal-wave demo ≥ 0.954 SSIM per page (CI threshold
0.94). The remaining demo differences are small horizontal drifts inside
lines (glyph advances/kerning), the Phase 1 width check.
