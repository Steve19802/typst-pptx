# Writing Touying themes for typst-pptx

How to write a Touying theme whose decks export to PowerPoint editable and
looking like the PDF. Every rule comes from how typst-pptx 0.3.0 maps
Typst's output (measurements in `docs/phase0-findings.md`). A deck that
breaks a rule still exports: the content comes out as a picture, as plain
text boxes or as shapes instead of the native PowerPoint object, and
`--report` says which and where.

## 1. Setup

- **Typst 0.15.1, Touying 0.8.x** (`@preview/touying:0.8.0`). Touying 0.7
  decks are rejected (exit 2).
- Keep the theme's fonts in the project (e.g. `fonts/`) and compile with
  `--font-path`, so PDF and PPTX use the same fonts.
- Import the hints package (§8):
  `#import "@local/pptx-hints:0.1.0" as pptx`.
- Exporter-only tweaks, if ever needed: typst-pptx sets
  `sys.inputs.export = "pptx"` (`sys.inputs.at("export", default: none)`).
  Avoid them where possible: the PDF and the PPTX should be the same deck.

## 2. Check loop

```sh
typst compile --font-path fonts demo.typ demo.pdf
typst-pptx compile --font-path fonts --report report.json demo.typ demo.pptx
```

A theme is in good shape when, on a demo deck that uses every slide kind:

- stderr has no `unknown font family` and no `font "…" is not embedded`;
- `report.json`: `baked` holds only equations (`"reason": "equation"`),
  and `tables_as_shapes`, `lists_as_text`, `line_mode` are empty;
- `--emit-ir ir.json` shows a `layouts` entry per slide kind and, with
  the hints, a `role` on titles, footers and slide numbers.

Every stderr warning and report entry carries the source position.

## 3. Fonts

| Rule | Why |
|---|---|
| Use fonts whose licence allows embedding (OpenType `fsType` not restricted) | Fonts are embedded in the `.pptx`; restricted ones are skipped with a warning and PowerPoint substitutes them |
| Prefer TrueType (`.ttf`); OpenType/CFF (`.otf`) works but is converted and larger | Only TrueType embeds in PowerPoint; ~0.2 MB per IBM Plex face, ~1 MB per New Computer Modern face |
| Ship every weight and style the theme uses as its own face (Regular, Medium, SemiBold, Bold, Italic …) | A weight that falls back to another face looks different in PowerPoint |
| If decks are edited in PowerPoint for the web, use Microsoft cloud fonts (IBM Plex is one; Libertinus and New Computer Modern aren't) | The web editor substitutes other fonts on its canvas, embedded or not |
| One family for headings (titles ≥ 24 pt) and one for body text | They become the PowerPoint theme's heading and body fonts (§7) |

## 4. Text

| Rule | Why |
|---|---|
| Text fills: solid colours (transparency is fine) | Gradient or tiling text is baked into a picture |
| Rotate text if needed, but don't scale, skew or mirror it | Rotation is native; the others are baked |
| Left-aligned or justified body text | Becomes one editable paragraph; centred/right-aligned multi-line paragraphs are kept line by line (exact look, less convenient to edit) |
| Put a link on a whole text block (e.g. a whole outline entry) rather than on a few words | A link on the whole box isn't underlined; PowerPoint always underlines links on part of a text |
| Don't rely on `clip: true` to hide parts of text or shapes | Only images are cut to a clip; clipped text and shapes show in full |

## 5. Shapes, images and SVG

| Rule | Why |
|---|---|
| Draw decorations with Typst shapes: `rect` (also with uniform `radius`), `circle`, `ellipse`, `line`, `polygon`, `curve` | Native, editable PowerPoint shapes |
| Gradients: linear, or radial centred on the shape, relative to the shape itself (the default) | Conic, off-centre radial and `relative: "parent"` gradients are baked |
| No tilings (`tiling(...)`) as fills | Baked |
| Strokes: any thickness, dash, cap and join | Native (a dash phase is dropped) |
| Small SVG decorations: only groups and paths, solid fills/strokes, ≤ 60 elements | Become native shapes; larger or richer SVGs stay pictures |
| No SVG `<pattern>`, `<filter>`, `<mask>`, `<foreignObject>`, `<text>` | The SVG becomes a PNG only (PowerPoint's SVG rendering of these is unreliable) |
| Raster images: no rotation inside a clipped box | Rotated + clipped images are baked |

## 6. Content elements

| Rule | Why |
|---|---|
| Equations are always pictures (alt text = source); keep them out of table cells | Any equation, image, shape or nested table in a cell turns the whole table into shapes and text boxes |
| Tabular data with `table`, one text block per cell, no `gutter`, no rotation, strokes on the grid lines | Becomes a native PowerPoint table (no row growth) |
| Lists with `list`/`enum`: one-character bullets, arabic or alphabetic numbering (`1.`, `a)`, `(1)`, `A.` …) that counts up (roman numerals become plain text) | Native bullets and numbering, editable with Enter and Tab |
| Speaker notes with Touying's `#speaker-note` | Become PowerPoint notes |

## 7. Slide chrome → slide layouts and theme

typst-pptx moves content that is **identical on several slides** (same
content, position and size) to a shared PowerPoint slide layout, one per
slide kind (title, section, content, focus …), and builds the PowerPoint
theme from the colours and fonts the deck uses.

| Rule | Why |
|---|---|
| Draw each slide kind's background and decorations the same way on every slide of that kind | Only exact repeats are shared; one varying parameter keeps the item on each slide |
| Page colour via the page `fill`; decorations via `page.background` or `place` at the **start** of the slide function | A layout is always drawn below the slide's content: an item moves only if nothing that stays on the slide is drawn below it overlapping it (text excepted) |
| Keep slide-dependent things (title, page number, progress bars) separate from static decorations | A bar whose length depends on the page can't be shared |
| A small palette: one background, one or two dark text colours, one light card colour, up to six saturated accents | Becomes the theme's colour scheme: background = lt1, the most used dark text colours = dk1/dk2, another light colour = lt2, saturated colours = accent1–6 |
| Use the accents in shapes and lines, not only in text | Accents used as fills or lines rank first; text-only colours (e.g. code highlighting) come after |
| Avoid near-duplicate shades of one colour | Each distinct colour competes for an accent slot; colours equal to a theme colour are written as theme references (recolourable in PowerPoint) |

## 8. Placeholders with `pptx-hints`

Marked text becomes a real PowerPoint placeholder: titles appear in
Outline view, footer and slide number work with Insert → Header & Footer,
the slide number updates when slides move, and LibreOffice treats them as
its own footer and slide number. Invisible in the PDF.

```typ
#import "@local/pptx-hints:0.1.0" as pptx

// content slide header
#pptx.role("title", self.store.title)        // or the current heading
// footer
#pptx.role("footer", self.store.footer)
// slide number: only the counter; " / N" stays outside
#context [#pptx.role("slide-number", utils.slide-counter.display()) / #utils.last-slide-number]
// title slide
#pptx.role("title", info.title)  #pptx.role("subtitle", info.subtitle)
// section slide
#pptx.role("title", utils.display-current-heading(level: 1, numbered: false))
```

Roles: `title`, `subtitle`, `body`, `footer`, `date`, `slide-number`.

| Rule | Why |
|---|---|
| Wrap only the text, not its decorations (lines, icons, boxes) | The role covers the text it encloses |
| One of each role per slide | Later ones become plain text boxes |
| One format per role (one font, size, colour; no mixed runs) | The placeholder carries that format; mixed formatting stays on each slide |
| Footer, date and slide number at the same position on every slide of a kind | Their frame is shared with the layout placeholder; LibreOffice uses its own footer/number only when they match |
| Slide number: mark only the number | Neither PowerPoint nor LibreOffice has a total-slides field; text around the number (" / 17") becomes fixed layout text next to it. The number is right-aligned at its right edge |
| Mark the title on every slide kind that has one (title, section, content) | Each kind's layout gets a title placeholder |

## 9. Animations (Touying subslides)

| Rule | Why |
|---|---|
| Use Touying's `#pause`, `#meanwhile`, `#uncover`, `#only`, `#alternatives` | Become click animations on one slide |
| Built-in covers (default `hide`, `alpha-changing-cover`, `color-changing-cover`), or covers that answer `utils.cover-kind-query` | The exporter needs to know how covered content is drawn |
| Revealing content must not move content already shown; keep `#only`/`#alternatives` branches the same size | Same layout → click animations; a layout change splits the slide into consecutive slides entered with Morph |
| Lists revealed item by item (`#pause` between items) | Become a by-paragraph build of one native list |

## 10. Summary for a theme review

1. Fonts embeddable, all faces in the project.
2. Only equations baked; no tables as shapes, no lists as text.
3. Decorations: Typst shapes or small path-only SVGs, drawn first.
4. Same chrome on every slide of a kind; slide-dependent parts separate.
5. Small palette, accents used in shapes.
6. `pptx.role` on titles (all kinds), subtitle, footer, slide number
   (number only).
7. Subslides that don't move existing content.
