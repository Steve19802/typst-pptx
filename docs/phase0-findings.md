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

## Baseline offset (plan §5.2), first measurement

Line-mode export of the signal-wave demo, text boxes with zero insets, single
spacing, box top = Typst baseline − Typst `ascender`. Typst's `ascender` is the
OS/2 typo ascender when present (IBM Plex Sans: 0.780 em; hhea/win ascent
1.025 em, `USE_TYPO_METRICS` off). LibreOffice 25.2 then draws every baseline
**0.218–0.220 em lower** than Typst (sizes 14–24 pt), i.e. its first baseline
sits ≈1.00 em below the box top. x positions match within 0.02 pt.
Undecided between formulas (hhea/win ascent would give 1.025 em); needs a
calibration deck with fonts whose typo/hhea/win metrics differ, then the
Windows check against PowerPoint.

## Touying metadata (plan §6.1)

`typst query <file> "<pdfpc-file>" --field value --one` returns
`{pdfpcFormat: 2, disableMarkdown, pages: [{idx, label, overlay,
forcedOverlay, hidden, note?}]}`; `note` is plain text. The adapter will read
the same metadata through the introspector.
