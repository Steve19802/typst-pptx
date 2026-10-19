# Plan: Editable PPTX export for Touying decks (`typst-pptx`)

Status: agreed v4 (ready for Phase 0) · Date: 2026-10-19 · Owner: Esteban Pupillo <esteban@egpworld.com.ar>
Targets: Typst 0.15.x (tino Dockerfile pin 0.15.1), **Touying ≥ 0.8.0, < 0.9**,
PowerPoint releases from the last 5 years (see §9).

## 0. Decisions taken

| # | Question | Decision | Consequence in this plan |
|---|---|---|---|
| 1 | Baseline theme | seaslides `custom-rich/signal-wave` as a **guideline**, not a hard requirement | It is the reference fixture (ported to Touying 0.8) and shapes the mapping (§3); the real targets are the owner's own themes (§7.1) |
| 2 | Text mode | **Exact look** | Typst line breaks are kept; PowerPoint never re-wraps (§5.2) |
| 3 | Subslides | **Animations** | `#pause`/`#uncover`/`#only` become click animations; layout shifts use Morph (§6) |
| 4 | Where it lives | **New repository**, binary consumed by tino | Separate release pipeline; tino downloads the binary like `typst` (§8, §10) |
| 5 | MCP and API | **Both** | REST endpoint + MCP tool `compile_pptx` (§10) |
| 6 | Compatibility | Last 5 years | PowerPoint 2021, 2024, Microsoft 365; Windows is verified, Mac/web best effort (§9) |
| 7 | Owner | Esteban Pupillo (esteban@egpworld.com.ar) | Owns the new repo and its releases |
| 8 | Touying version | **0.8.x only** | 0.7.x decks are rejected with an upgrade hint; signal-wave is ported to 0.8 (§6.1) |
| 9 | Own themes | A personal and a work theme, built later | Theme-authoring rules + starter theme in the new repo (§7.1) |
| 10 | PowerPoint access | One Windows machine, milestones only | Iterations are verified automatically; manual checks at milestones (§11) |

| 11 | Repository | `typst-pptx` on GitHub, owner's account | §8, §10 |
| 12 | Licence | Apache-2.0 | `LICENSE` + `NOTICE` with MIT attributions |

No open questions remain (§13).

## 1. Goal and non-goals

Produce a PowerPoint file from a Typst/Touying deck that **looks like the Typst
PDF** and where text, shapes, tables, pictures, notes and click steps are
**native, editable PowerPoint objects**. No PDF/SVG document is used as an
intermediate: a Rust CLI compiles the deck with the `typst` crate and maps the
laid-out frame tree straight to PresentationML/DrawingML.

The ppt-master mapping
(https://github.com/hugohe3/ppt-master/blob/main/docs/powerpoint-svg-mapping.md, MIT)
is the **specification of the PowerPoint side**: which DrawingML construct is
native/normalized/approximate/bake-required, units, limits. Its SVG→DrawingML
rules are reused literally where the input *is* SVG (the theme's inline SVG
decorations, §5.5). Code may be ported from ppt-master's `pptx_*` scripts with
MIT attribution, but nothing at runtime depends on Python.

Non-goals for v1: PPTX → Typst import; math as native OMML equations; Typst
bundle documents; perfect behaviour *after* the user edits text (exact look
means text boxes don't re-wrap); `show-notes-on-second-screen` decks.

## 2. Architecture

```
tino (Python)                                       typst-pptx (Rust CLI, new repo)
┌───────────────────────────────────────┐          ┌──────────────────────────────────────────────────┐
│ GET <bucket>/compile/pptx/{path}      │          │ 1 World: root, fonts, packages, sys.inputs       │
│ MCP compile_pptx                      │──exec──▶ │ 2 typst::compile::<PagedDocument>                │
│ CompilerService.compile_pptx()        │          │ 3 Touying adapter: pdfpc metadata → logical slide│
│   _build_pptx_cmd() (shares font/pkg) │          │   + subslide groups                              │
│   parse_diagnostics() (same format)   │◀─file──  │ 4 Frame walker + tag tracker → IR (per page)     │
│ export split button "PowerPoint"      │  +stderr │ 5 Subslide diff → animation timeline / Morph     │
└───────────────────────────────────────┘          │ 6 Layout extraction (repeated decorations)       │
                                                   │ 7 Fallback renderer (typst-svg / typst-render)   │
                                                   │ 8 OOXML writer → .pptx (+ report.json)           │
                                                   └──────────────────────────────────────────────────┘
```

### 2.1 Repository layout (new repo, Cargo workspace)

```
typst-pptx/
  Cargo.toml                    # workspace; typst crates pinned with "=0.15.1"
  crates/
    pptx-ir/                    # intermediate representation, serde (JSON dump for tests)
    typst-frontend/             # World impl (typst-kit fonts+packages), compile, frame walker, tag tracker
    touying-adapter/            # pdfpc metadata → logical slides, notes, subslide groups
    animate/                    # subslide diff, timeline builder, Morph splitting
    layouts/                    # repeated-content detection → slide layouts, role hints
    fallback/                   # subframe → SVG (typst-svg) + PNG (typst-render)
    ooxml/                      # PresentationML/DrawingML writer (quick-xml + zip), font embedding
    cli/                        # bin `typst-pptx`
  typst/pptx-hints/             # tiny Typst package: role markers (§7)
  themes/starter/               # Touying 0.8 starter theme for the owner's own themes (§7.1)
  docs/theme-authoring.md       # exporter-friendly theme rules (§7.1)
  fixtures/                     # feature decks, signal-wave (ported to 0.8), expected IR/XML snapshots
  tools/validate/               # LibreOffice render + image diff, OOXML schema check, python-pptx open
  .github/workflows/            # ci.yml, release.yml, typst-update.yml
```

Main dependencies: `typst`, `typst-library`, `typst-kit` (fonts, package
download, rustls so musl builds work), `typst-svg`, `typst-render`, `usvg`
(the version Typst uses), `quick-xml`, `zip`, `png`, `ttf-parser`/`skrifa`
(metrics, embedding checks), `serde`, `clap`, `codespan-reporting` (same
diagnostics output as the typst CLI), `insta` (snapshots).

### 2.2 Intermediate representation (pptx-ir)

`Deck { size, fonts[], media[], layouts[], slides[] }`;
`Slide { layout_ref, items[], notes, timeline, transition }`;
`Item = TextBox | Shape | Line | Picture | Table | Group`, each with
`id`, `name` (stable, used for Morph `!!name`), EMU geometry, transform,
`source` (file/span for warnings) and `fidelity` (`native | normalized |
approximate | baked`, the ppt-master vocabulary). The IR is dumped as JSON with
`--emit-ir` and is the main snapshot-test surface.

## 3. Reference theme: signal-wave inventory

Source: `touying-typ/seaslides/skills/seaslides-typst-slides-skill/templates/themes/custom-rich/signal-wave`
(`template.typ`, 734 lines; `demo.typ`, 369 lines). It is a **guideline**: it
exercises most of what a rich theme does (SVG decorations, cards, tables,
code, math, notes), so it is the main fixture, but nothing in the exporter is
specific to it. Findings that drive the plan:

- Pins **Touying 0.7.4**. Phase 0 ports it to 0.8 as `fixtures/themes/signal-wave/`
  (MIT, attribution kept). Expected effort is small: it uses
  `touying-slide-wrapper`, `components.cell`, `utils.call-or-display`,
  `utils.display-current-heading`, `utils.slide-counter`,
  `utils.last-slide-number`, `lazy-v`, `cols` — each checked against 0.8,
  where several helpers moved out of `utils`.
- All decorations are **inline SVG strings** placed with `image(bytes(...))`,
  most with `preserveAspectRatio="none"` (non-uniform scaling).
- The demo uses **mitex** (LaTeX → Typst math), `$...$` inline math, `cols`
  with `lazy-layout`, `comparison-table`, `code-block` (raw with syntax colours),
  14 `#speaker-note`s, and **no `#pause`/`#only`** → we must write our own
  animation fixtures in this theme.

| Theme element | What Typst produces in frames | PPTX target | Fidelity |
|---|---|---|---|
| Page fill `palette.bg` | page fill | `p:bg` solid on the layout | native |
| `_oscilloscope-grid-svg` (uses `<pattern>` + SVG `<text>`) | Image(SVG) full-page | Picture (SVG + PNG fallback) on **layout** | baked (pattern, SVG text) |
| `_sine-waves-svg` | Image(SVG) full-page | Picture on layout (option: native freeforms) | baked / normalized |
| `_waveform-frame-svg`, `_antenna-tower-svg` | Image(SVG) | Picture on title/ending layout | baked / normalized |
| `_spectrum-bars-svg`, `_wave-divider-svg`, `_pulse-line-svg` (rects/paths, opacity, rx) | Image(SVG) small | **Native shapes** via usvg → DrawingML (ppt-master rules), group | normalized |
| `_noise-pattern-svg` in `box(clip: true)` | Image(SVG) + clip | Picture | baked |
| Header title (heading level 2, semibold 1.2em) | Text items in header frame | Title placeholder on layout + slide text (§7) | native |
| Header rule `line(...)` | Shape Line | `p:cxnSp` straight connector on layout | native |
| Footer text, wave icon, `n / N` counter | Text + Image | footer placeholder, picture/shape, `sldNum` field + static " / N" | native |
| `frequency-card`, `pulse-card` (stacked blocks, top-only radius, per-side strokes) | Rect/Path fills + separate stroke paths | `round2SameRect` / `custGeom` fills + line shapes, grouped | normalized |
| `spectrum-box`, `insight-box`, `channel-card`, `stat-card` | same family | same | normalized |
| `signal-tag` (inline box with icon) | box Shape + Image + Text | rounded rect + shapes + text box, grouped | normalized |
| `code-block` (terminal bg, coloured dots, raw text) | Rect, 3 ellipse paths, Text runs with colours | rect, `ellipse` prstGeom, text box with coloured runs | native |
| `comparison-table` (alternating fills, bottom rules only) | table frames + tags | `a:tbl` with cell fills/borders (§5.6) | normalized |
| mitex / `$...$` math | glyphs from math font, lines | Picture (SVG + PNG) — inline math: see §5.3 | baked |
| `focus-slide` (primary fill, stronger waves) | page fill + Image | own layout, bg solid + picture | native/baked |
| `outline-slide` (links to sections) | Text + Link items | text with `hlinkClick` slide jumps | native |
| `#speaker-note` | pdfpc metadata | notes slide | native |

## 4. Typst side: compile and walk

1. **World**: `--root`, `--font-path` (several dirs, OS path separator, same
   order as tino), `--package-path`, `--package-cache-path`, `--input k=v`,
   `--creation-timestamp`. The exporter always adds `sys.inputs.export =
   "pptx"` so decks can adapt (e.g. disable second-screen notes).
2. **Compile** with `typst::compile::<PagedDocument>`; warnings/errors printed
   with codespan in the typst CLI format (tino's `parse_diagnostics` must parse
   them unchanged — covered by a tino test).
3. **Frame walker**: depth-first over `Frame` items, keeping the accumulated
   `Transform` and clip stack. Items: `Group` (transform, clip, label),
   `Text`, `Shape`, `Image`, `Link`, `Tag`.
4. **Tag tracker**: `Tag::Start/End` give element boundaries (paragraph, list
   item, heading, table cell, raw, equation, figure, link, labelled metadata).
   The tracker builds a stack so each text/shape knows its enclosing elements.
   This is how lines are grouped into paragraphs and how math/tables are
   detected. Phase 0 verifies exactly which elements carry tags in 0.15.1.
5. **Introspector** queries (no `typst query` process): `<pdfpc>`/Touying
   metadata, `pptx-hints` role metadata, headings for slide titles.

## 5. Mapping details (exact look)

Units: Typst `Abs` points → EMU (1 pt = 12 700 EMU), rounded once at write
time. Slide size = page size (16:9 Touying = 841.89 × 473.56 pt → 10 692 000 ×
6 014 212 EMU, custom `p:sldSz`).

### 5.1 Fonts

- Each `TextItem` names its font; fallback glyphs arrive as separate items →
  separate runs with explicit `a:latin`/`a:ea`/`a:cs` typeface.
- **Embed fonts** by default (`--embed-fonts auto`): TrueType-outline fonts
  whose OS/2 `fsType` allows embedding are subset-free embedded as obfuscated
  `.fntdata` parts (`p:embeddedFontLst`, regular/bold/italic/boldItalic). This
  is what makes "exact look" hold on a machine without IBM Plex Sans.
  CFF-outline (OTF) fonts and restricted fonts → warning in the report.
- Family/weight naming: PowerPoint knows only regular/bold/italic per family;
  weights like `semibold`/`medium` (signal-wave uses both) are addressed by
  the font's legacy family name (e.g. "IBM Plex Sans SemiBold"), taken from
  name ID 1/2, not the typographic family. Phase 0 checks this on Windows+Mac.

### 5.2 Text: keep Typst's lines

Default `--text exact`:
- One **text box per Typst paragraph** (from paragraph tags), `wrap="none"`,
  all insets 0, `anchor="t"`, no autofit, explicit `a:br` at every Typst line
  break, so PowerPoint never re-wraps.
- Line spacing = Typst baseline distance as exact `a:lnSpc/a:spcPts`;
  `spcBef/spcAft` 0. Box y is computed so PowerPoint's first baseline matches
  Typst's (offset formula from font ascent/descent and exact line spacing,
  calibrated in Phase 0 against PowerPoint and LibreOffice).
- Justified lines: Typst stretches spaces. Emit extra width on the space runs
  via `a:rPr@spc` (hundredths of a point), alignment `l`. Same technique for
  Typst's tracking (`text(tracking:)` → `spc`).
- Kerning: `kern="100"` (on from 1 pt). Ligatures: PowerPoint applies standard
  ligatures only in some builds — width drift is detected by the check below.
- **Width check**: for every line, compute the advance width PowerPoint will
  use (font `hmtx` + kerning) and compare with Typst's. If any line drifts more
  than `--tolerance` (default 0.5 % or 1 pt), that paragraph falls back to
  **line mode**: one text box per line at Typst's exact x/y. Still editable,
  just less convenient. Counted in the report.
- Runs: font, size, colour (+alpha from `transparentize`), bold/italic flags
  matching the embedded face, underline/strike (Typst draws decorations as
  shapes — mapped back to `u`/`strike` when they match the run, else lines),
  baseline shift (sub/superscript), links (`hlinkClick`).
- Lists: Typst draws markers as their own text/shapes. In exact mode markers
  become **native bullets** (`a:buChar`/`a:buAutoNum` with `marL`/`indent`
  taken from Typst positions) when marker glyph and indent match; otherwise
  the marker stays a separate run with a tab. `show-hide-set-list-marker-none`
  is respected (no marker on hidden items).
- Raw/code: same pipeline; syntax colours become run colours.

`--text flow` (opt-in, not default): one paragraph without `a:br`, wrap on,
for decks that will be heavily edited.

### 5.3 Math

- Block equations (`math.equation(block: true)`, all mitex output): rendered
  as a **picture** from the equation's subframe using `typst-svg` (SVG) plus
  `typst-render` (PNG fallback at 2× slide resolution). Pixel-identical to
  the PDF. `descr`/alt text = the source text (mitex source for mitex).
- Inline math (`$f_s$`): picture anchored at its line position, and the
  paragraph's text box gets a space-run of the same width so the line doesn't
  close up. Option `--inline-math text` maps simple inline math (letters,
  digits, sub/superscripts only) to italic runs with `baseline` offsets.
- Native OMML is a later, separate project (non-goal v1).

### 5.4 Shapes

| Typst geometry | DrawingML |
|---|---|
| `Geometry::Line` | `p:cxnSp` `prstGeom line` (flips for direction) |
| `Geometry::Rect` | `prstGeom rect` |
| `Geometry::Curve`/`Path` recognised as rounded rect (all/top/bottom radii) | `roundRect`, `round2SameRect`, `round1Rect`, `adj` from radius |
| Path recognised as 4-Bézier ellipse | `prstGeom ellipse` |
| Any other path | `custGeom` with `a:path` (moveTo/lnTo/cubicBezTo/close), fill rule note |
| Fill: solid, `transparentize` | `a:solidFill` + `a:alpha` |
| Fill: linear/radial gradient | `a:gradFill` (`lin`/`path circle`), stops; conic → baked |
| Fill: tiling | baked (picture) |
| Stroke: paint, thickness, cap, join, dash | `a:ln` `w`, `cap`, join, `prstDash`/`custDash` |

Typst's per-side block strokes come out as separate paths → separate line
shapes, grouped with the fill shape in a `p:grpSp` named after the component.

### 5.5 Images and the theme's SVG decorations

- Raster images → `p:pic` (PNG/JPEG/GIF as-is), crop via `a:srcRect` when a
  clip is a simple rect, de-duplicated by hash in `ppt/media`.
- SVG images: Typst keeps the SVG bytes in the `Image`. Policy `--svg auto`:
  1. Parse with usvg (as Typst does). If the tree only uses constructs that
     ppt-master marks Native-stable/Native-normalized (rect, circle, ellipse,
     line, polyline, path, solid fill/stroke, opacity, dasharray, simple
     gradients) **and** has ≤ `--svg-max-nodes` (default 60) nodes, convert to
     native shapes in a group, applying the viewBox→box transform incl.
     `preserveAspectRatio="none"` (non-uniform scale is folded into the path
     coordinates and stroke widths are scaled by the mean factor; a warning
     if the stroke ratio differs > 10 %).
  2. Otherwise (patterns, SVG `<text>`, filters, masks, big trees) → picture:
     `a:blip` PNG fallback + `asvg:svgBlip` extension with the original SVG.
- For signal-wave this gives: grid/sine/frame/antenna/noise → pictures on
  layouts; spectrum bars, dividers, pulse lines, tag icons → native shapes.

### 5.6 Tables

- Detect `table` via tags (cells with x/y/colspan/rowspan). Emit
  `p:graphicFrame` + `a:tbl` with column widths and row heights from Typst,
  cell fills, per-side borders (`lnB` etc.), cell margins 0 and text from the
  §5.2 pipeline (with `a:br`).
- Exact-look risk: PowerPoint grows a row if it thinks text doesn't fit. Rows
  are given Typst's height and text uses Typst's line breaks; Phase 3
  verifies no growth. Cells with non-text content (math, images, nested
  blocks) or a failed check → the table becomes a **group of shapes and text
  boxes** (`--tables auto|native|shapes`).
- `grid`/`cols` layouts are **not** tables; they are just positions.

### 5.7 Groups, clips, transforms, links

- Typst `Group` with transform → `p:grpSp` with `a:xfrm`/`chOff`/`chExt`;
  rotation and flips mapped; skew/non-uniform scale on text → baked picture.
- Clips: rectangular clip on a picture → `srcRect` crop; on shapes →
  geometry intersection where trivial, else baked; rounded clips → baked.
- Links: external URLs → `hlinkClick` r:id; internal (outline-slide) →
  `ppaction://hlinksldjump` to the target slide.

### 5.8 Fallback ("baked") rendering

Any subtree that can't be expressed is rendered from its own subframe with
`typst-svg`/`typst-render` and inserted as one picture at the exact position,
named `baked:<element>` and listed in `report.json` with source span and
reason. Fallbacks therefore never change the look, only editability.

## 6. Touying: slides, notes, animations

### 6.1 Adapter

- **Supported: Touying 0.8.x only.** Reasons:
  - 0.8.0 requires Typst 0.15, the version tino already pins; 0.7.x targets
    Typst 0.12, so tino would be testing an old pairing.
  - 0.8 changed the pause semantics (`#pause` after `uncover`/`only`/
    `alternatives` no longer skips reserved subslides). Supporting both means
    two diff behaviours for the same source.
  - 0.8 adds what the exporter needs: covers answer
    `utils.cover-kind-query` with `"hide"`, `"recolour"` or `"paint"`, so the
    diff knows how covered content is drawn; pdfpc metadata and heading
    lookups are scoped per document; `utils.get-input` reads `--input` values
    in a structured way; list animations are layout-stable.
  - The owner's own themes will be new, so there is no 0.7 code to keep.
- Detection: the World records every resolved package spec. If the deck
  imports `@preview/touying:0.7.*` or older, export stops with
  `error: typst-pptx needs Touying 0.8 or newer (found 0.7.4)` and a link to
  Touying's migration guide. Decks without Touying export as plain Typst (one
  page = one slide, no animations).
- Read Touying's per-page `<pdfpc>` metadata through the introspector (logical
  slide index, overlay/subslide index, notes). Phase 0 confirms its exact
  shape in 0.8.0 and freezes it in a test; the adapter fails clearly on an
  unknown shape (guards against 0.9 changes).
- The exporter passes `--input export-mode=presentation` (read by Touying
  0.8's `utils.get-input`) unless the user overrides it, so `handout`
  and `article` modes are never picked up by accident.
- Group pages into logical slides; notes are attached to the logical slide
  (`ppt/notesSlides`, plain paragraphs; markdown-ish line breaks kept).
- Hidden/handout handling: `handout: true` decks export as-is (one page = one
  slide). Second-screen-notes decks (page width ≈ 2× height ratio) → error
  with a hint to use `sys.inputs.export`.
- Bundles (Touying 0.8 multi-document) → error in v1.
- Article mode output (prose document) → error: not a slide deck.
- Covers: `"hide"` → content absent from the frame (entrance/exit);
  `"recolour"` (`alpha-changing-cover`, `color-changing-cover`) → content
  present with other paints, mapped to a colour/opacity change on click;
  `"paint"` (overlay rectangle) → the overlay shape gets an exit animation.

### 6.2 Subslide diff

For each logical slide with subslides `S1..Sn`:
1. Flatten every page to IR items with a **content key** = hash(kind, geometry
   rounded to 0.1 pt, paint, text, font, image hash) and the element path from
   tags.
2. Match items across consecutive subslides by key (exact position).
   Covered content under the default `hide` cover is absent from the frame;
   under `alpha-changing-cover` it is present with lower alpha → key ignores
   alpha and records an "opacity state".
3. Classify each step `Sk → Sk+1`:
   - **Additions only** (`#pause`, `#uncover`, `#meanwhile`) → entrance.
   - **Additions + removals, nothing else moved** (`#only`, `#alternatives`
     of equal size) → exit + entrance on the same click.
   - **Opacity change** (alpha cover) → opacity emphasis/entrance.
   - **Something moved or reflowed** (content inserted above, `#only` with
     different heights) → **layout shift**.

### 6.3 Output

- Steps without layout shift: one PowerPoint slide containing the union of
  items, a `p:timing` tree with one click group per step: `presetClass="entr"
  presetID="1"` (Appear) / `"exit" presetID="1"` (Disappear), items that are
  both added and part of the same paragraph are animated **by paragraph**
  (`bldP` with `build="p"` and paragraph ranges) so a list revealed with
  `#pause` stays one text box.
- Layout shift: the logical slide is split at that step into consecutive
  PowerPoint slides; the following slide gets a **Morph** transition
  (`p159:morph` inside `mc:AlternateContent`, fallback `p:fade`), and items
  that persist get the same `!!name` so Morph pairs them. Notes go on the first
  slide of the split. Morph needs PowerPoint 2019+/365 — inside the window.
- `--animations auto` (default) as above; `none` exports only the last
  subslide; `slides` exports every subslide as its own slide (no timing).
- Slide numbers: Touying keeps the logical number on all subslides, so the
  `sldNum` field + " / N" stays correct in all modes except `slides` (there the
  static text from Typst is kept).

## 7. Slide masters and layouts

- **Automatic** (no deck changes): items that are identical (content key) on
  every page produced by the same slide function and lie behind the content
  (first in z-order) are moved to a **slide layout**. Pages are clustered by
  their repeated-item sets → for signal-wave: content, title/ending, section,
  focus, outline layouts. One master holds the theme fonts and colour scheme
  (from `config-colors`: primary → accent1, secondary → accent2 …, read via the
  palette if the deck exposes it, else from the most frequent paints).
- **Hinted** (opt-in, small Typst package `pptx-hints` in the new repo):
  `#pptx.role("title")[..]`, `role("footer")`, `role("slide-number")`,
  `role("subtitle")`, `role("body")`, implemented as labelled metadata around
  the content, readable via tags. Roles become `p:ph` placeholders
  (title/subTitle/body/ftr/sldNum/dt) on the layout with the slide's text in
  the matching placeholder. The 0.8 port of signal-wave in `fixtures/` gets
  the roles too (header/footer, about 10 lines), as a worked example.
- Without hints the header title is still a normal text box — fully editable,
  just not a placeholder.

### 7.1 Own themes: authoring guide and starter theme

The owner will write a personal and a work theme later, so the new repo ships
`docs/theme-authoring.md` and `themes/starter/` (Touying 0.8, roles already
wired) that both themes can start from. Rules, each tied to an exporter
behaviour:

| Rule | Why |
|---|---|
| Use TrueType (`.ttf`) fonts whose licence allows embedding | Only these embed reliably (§5.1); exact look depends on it |
| Prefer Typst shapes (`rect`, `circle`, `line`, `curve`, gradients) over SVG for small decorations | Always native; SVG goes through the usvg filter (§5.5) |
| Put large or patterned decorations (grids, textures, photos) in the page background or a `place` at the start of the slide function | They are detected as layout content and stored once (§7) |
| Avoid SVG `<pattern>`, `<text>`, filters, masks, and rounded/clipped boxes around content that should stay editable | They are baked into pictures (§5.5, §5.7) |
| Wrap title, subtitle, footer and slide number in `pptx.role(...)` | They become real PowerPoint placeholders and fields (§7) |
| Expose the palette through `config-colors` (primary, secondary, tertiary, neutrals) | Becomes the PowerPoint colour scheme, so recolouring works in PowerPoint |
| Use Touying covers that answer `cover-kind-query` (all built-in ones do) | The animation diff relies on it (§6.1) |
| Keep `#only`/`#alternatives` branches the same size where possible | Same-size branches become click animations; different sizes become Morph slides (§6.3) |
| Use `utils.get-input("export")` for exporter-only tweaks | The exporter sets `export=pptx` (§4) |

A `typst-pptx lint` subcommand reports rule violations for a deck (baked
items, non-embeddable fonts, missing roles, size-changing `#only`) using the
same report data, so a new theme can be checked without PowerPoint.

## 8. CLI contract (consumed by tino)

```
typst-pptx compile INPUT [OUTPUT.pptx]
    --root DIR --font-path P1:P2 --package-path DIR --package-cache-path DIR
    --input KEY=VAL ... --creation-timestamp TS
    --animations auto|none|slides      (default auto)
    --text exact|flow                   (default exact)
    --tables auto|native|shapes         (default auto)
    --svg auto|picture|native           (default auto)
    --embed-fonts auto|never            (default auto)
    --report FILE.json --emit-ir FILE.json
    --diagnostic-format human|short
typst-pptx --version   →  typst-pptx 0.1.0 (typst 0.15.1)
```

- Exit 0 = success (warnings allowed), 1 = compile error, 2 = export error
  (e.g. unsupported bundle). Errors/warnings on stderr in typst CLI format;
  export problems are `warning:` with the element's source span.
- `report.json`: per slide, counts of native/normalized/baked items, fallback
  reasons with spans, font embedding results, animation mode per slide.
- Deterministic output (sorted parts, fixed zip timestamps) for snapshot tests.

## 9. PowerPoint compatibility (last 5 years)

Targets: PowerPoint 2021, 2024 and Microsoft 365. **Verified** on the owner's
Windows machine at milestones (§11). Mac, PowerPoint for the web,
PowerPoint 2019 and LibreOffice Impress 7.6+ are **best effort**: the file only
uses constructs documented for them, but nobody checks them by hand.
PowerPoint for the web can be opened from Linux in a browser with a free
Microsoft account, so it can be spot-checked if needed.

| Feature used | Needs | Fallback in file |
|---|---|---|
| ECMA-376 Transitional parts, DrawingML shapes, tables, notes, classic `p:timing` | all versions | — |
| SVG pictures (`asvg:svgBlip`) | 2016+ / 365 | PNG blip always present |
| Morph transition (`p159:morph`) | 2019+ / 365 / web | `mc:AlternateContent` → fade |
| Embedded fonts | Windows all; Mac 16.17+; web: verify in Phase 0 | system font substitution (look degrades, report says so) |
| `a16:creationId` etc. | not used | — |

Rules: no 3D, Zoom, Designer, ink or `p14`-only animations; every extension
inside `mc:AlternateContent` or `extLst` with a fallback; the writer validates
against ECMA-376 transitional XSDs in CI.

## 10. tino integration

Binary distribution (decision 4):
- `Dockerfile`: `# renovate: github-releases depName=<org>/typst-pptx`
  `ARG TYPST_PPTX_VERSION=…`; download `typst-pptx-${TYPST_TARGET}.tar.xz` in
  the same `RUN` using the existing `TYPST_TARGET` switch. The new repo must
  publish exactly these targets: `x86_64-unknown-linux-musl`,
  `aarch64-unknown-linux-musl`, `armv7-unknown-linux-musleabi`,
  `riscv64gc-unknown-linux-gnu`.
- Version coupling: `typst-update.yml` gets a step that finds the newest
  typst-pptx release built for the new Typst version and bumps both ARGs in
  the same PR; if none exists yet, the PR body says so and a CI check
  (`typst --version` vs `typst-pptx --version` in the built image) fails, so
  the two can't drift silently.
- Dev setups without the binary: export is reported as unavailable, not an
  error at startup.

Backend:
- `tino/config.py`: `TINO_PPTX_BINARY` (default `typst-pptx`, resolved with
  `shutil.which`; unset/missing → feature off).
- `tino/services/compiler.py`: `pptx_version()` (cached like
  `typst_version()`), `CompilerService.compile_pptx(slug, path, animations)`
  modelled on `compile_pdf` (temp file, caller deletes), `_build_pptx_cmd()`
  sharing font-path/package-path logic with `_build_cmd()`, same
  `_strip_paths` + `parse_diagnostics` + `CompileError`, a longer timeout
  (exports with many fallbacks render PNGs). Report JSON is read and returned.
- `tino/routers/compile.py`: `GET /pdf`-style
  `GET .../compile/pptx/{path}?animations=auto|none|slides` → `FileResponse`
  `application/vnd.openxmlformats-officedocument.presentationml.presentation`,
  `X-Tino-Pptx-Fallbacks` header with the baked count; 501 when unavailable.
  This is also the REST API (API keys already work on these routes).
- `tino/routers/misc.py`: `/health` and `/api/config` add `pptx_version`
  (null when unavailable).
- `tino/mcp/server.py`: `compile_pptx(bucket, path, animations="auto")` →
  `{success, typst_pptx_version, pptx: <base64>, size, report}`, viewer role,
  same error shape as `compile_pdf`. `tino/mcp/instructions.py`: one line on
  when to use it (and to read `report` to see what was baked).

Frontend:
- `tino/static/js/preview-manager.js`: third format `pptx` in the split
  button ("Generate PowerPoint"), `downloadPptx()`, shown only when
  `/api/config` reports `pptx_version`; animation mode as a sub-option
  (default auto). Toast with fallback count from the response header.
- `tino/static/js/api.js`: no change if downloads stay URL-based.

Docs and tests:
- `docs/usage/editor.rst` (export), `docs/usage/api.rst`, `docs/usage/mcp.rst`
  (tool table), `docs/operations/deployment.rst` (binary, env var),
  `docs/development/adr/11-pptx-export.rst` (why a separate Rust exporter, why
  no PDF/SVG intermediate, version coupling).
- tests: command building, diagnostics parsing with captured typst-pptx
  stderr, router 200/404/422/501, MCP tool success/failure, config exposure;
  the binary itself is faked in unit tests, one integration test runs when the
  binary is present.

## 11. Testing and acceptance (new repo)

Fixtures:
- **Feature decks** (plain Typst + Touying simple theme): text styles,
  justification, tracking, lists (nested, enum), links, shapes, gradients,
  images, tables, math, groups/rotation/clip, `#pause`, `#uncover`, `#only`,
  `#alternatives`, `#meanwhile`, alpha cover, notes.
- **signal-wave decks** (ported to Touying 0.8): the upstream `demo.typ`
  with only the import changed; `animations.typ` (every slide type and
  component with pause/only/alternatives and each cover kind); the
  role-hinted variant.
- **starter theme** deck (§7.1); later the owner's personal and work themes.
- A Touying 0.7.4 deck that must fail with the upgrade message.

PowerPoint is not available during iterations, so everything that runs per
commit is automatic, and PowerPoint itself is used only at milestones.

Automatic checks per fixture in CI (and locally via `make check`):
1. Snapshot of IR JSON and of normalised slide XML (`insta`).
2. Package validity, in rising strictness:
   - OPC structure and content types; python-pptx opens the file.
   - ECMA-376 Transitional XSD validation of every XML part.
   - **Open XML SDK validator** (.NET, runs on Linux) in PowerPoint 2021
     mode. This catches most problems that make PowerPoint show the "repair"
     prompt; any error fails CI.
   - ppt-master's MIT `pptx_opc_validation.py`/`pptx_delivery_check.py` as an
     extra opinion (vendored test tool, not a runtime dependency).
3. **Visual diff**: LibreOffice headless renders each slide (final animation
   state) to PNG and it is compared with `typst-render` of the last subslide
   (SSIM threshold tuned in Phase 0).
4. **Text-fit check**: the exporter's own width model (§5.2) is checked
   against LibreOffice's text layout per line, to catch metric mistakes early.
5. **Timing check**: the `p:timing` tree is parsed back and compared with
   the expected click sequence derived from the subslides.

LibreOffice is only a proxy for PowerPoint, especially for text metrics and
animations. That is why there is:

**Milestone check on Windows** (end of Phases 0, 1, 4 and 6, ~1 h each): a
`checklist.md` and a zip of fixture decks with a reference PDF per deck. The
owner opens each deck and records pass/fail and screenshots in an issue: no
repair prompt, fonts embedded and used, text lines match the PDF, no table row
growth, animations step correctly, Morph pairs objects, notes present, text
editable. Findings become new fixtures or tolerance changes.

Acceptance for v1 (signal-wave demo + animations deck):
- Every slide visually matches the PDF (SSIM threshold + manual check).
- ≥ 95 % of visible text characters are in editable text boxes (math excluded).
- All cards, tags, dividers, code blocks, tables are native objects.
- All 14 speaker notes present on the right slides.
- Pause/only/alternatives step correctly in PowerPoint slideshow.
- Zero Open XML SDK validator errors; opens without a repair prompt in
  PowerPoint on the owner's Windows machine.

## 12. Phases

| Phase | Scope | Exit criterion | Estimate |
|---|---|---|---|
| 0 Spike | New repo, port signal-wave to Touying 0.8, World, compile, frame walk → text boxes (exact mode), rects, pictures, minimal pptx writer, CI with the validators of §11; verify tag coverage in Typst 0.15.1, Touying 0.8 pdfpc metadata shape, baseline formula, font naming for semibold | signal-wave demo passes the automatic checks; Windows milestone check on 5 sample slides | 1.5 weeks |
| 1 Core mapping | §5.1–5.5, 5.7, 5.8 incl. SVG→native, width check + line mode, report, CLI contract, diagnostics | demo passes visual diff, report lists only math as baked | 2 weeks |
| 2 Release + tino | release workflow (4 targets), typst-update workflow in new repo; tino Dockerfile, service, router, MCP, UI, docs, ADR (with `--animations none`) | export from tino UI/API/MCP works in the Docker image | 1 week |
| 3 Tables, lists, notes | §5.6, native bullets, notes, outline links | comparison-table native with no row growth; notes OK | 1.5 weeks |
| 4 Animations | §6.2–6.3: diff, timing XML, by-paragraph builds, Morph splitting | animations fixture correct in PowerPoint 365 + 2021 | 2 weeks |
| 5 Layouts and hints | §7 automatic layouts, theme colours/fonts, `pptx-hints` package, patched signal-wave | layouts deduplicate backgrounds; placeholders with hints | 1.5–2 weeks |
| 6 Themes and hardening | starter theme, theme-authoring guide, `lint` subcommand (§7.1); Touying's bundled themes as extra fixtures; perf (large decks); inline-math text option | v1.0 release; Windows milestone check | 1.5 weeks |

Total ≈ 11–12 weeks for one developer. The personal and work themes are built
after v1 on top of the starter theme and become fixtures as they appear. tino gets a usable export after Phase 2
(exact look, no animations yet); animations land with Phase 4 without tino
changes beyond enabling the default.

## 13. Open questions

None. Resolved on 2026-10-19:
1. Repository: `typst-pptx`, hosted on GitHub under the owner's account
   (`<owner>/typst-pptx`; the exact account slug goes into the release
   workflow and tino's renovate comment).
2. Licence: **Apache-2.0** (same as the Typst crates; compatible with tino's
   AGPL and with MIT code ported from ppt-master and Touying, which keeps its
   attribution in `NOTICE`).

## 14. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| PowerPoint's text metrics differ from Typst's | lines overflow/shift | width check → line mode; embedded fonts; calibration fixtures |
| Typst internal API (frames, tags, introspector) changes | breakage on every Typst bump | `=` version pin, typst-update workflow in the new repo, snapshot tests flag diffs |
| Touying metadata format changes (0.8 → 0.9) | wrong subslide grouping | `< 0.9` bound, metadata shape frozen in a test, clear error on unknown shape |
| 0.7.x decks (e.g. seaslides templates) can't be exported | users must upgrade | explicit error with migration link; signal-wave port documents the steps |
| No PowerPoint during iterations | regressions found late | Open XML SDK validator + LibreOffice diff per commit; Windows checks at 4 milestones |
| Mac/web not verified | unnoticed issues there | standard constructs with fallbacks only; web spot-check possible from Linux |
| Font embedding limits (CFF, fsType, web) | look degrades on other machines | report + docs; recommend TTF fonts |
| Table row growth in PowerPoint | layout change | row checks, `shapes` fallback |
| Morph pairing surprises | odd transitions | stable `!!` names from element paths; `--animations slides` escape hatch |
| Large decks with many baked items | slow export, big files | media dedup, PNG at 2× only, timeout config in tino |
