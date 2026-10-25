# pptx-hints

Marks the parts of a slide that typst-pptx should export as PowerPoint
placeholders (title, subtitle, body, footer, date, slide number) and
declares the theme's palette for PowerPoint's colour scheme.

```typ
#import "@local/pptx-hints:0.2.0" as pptx

#pptx.role("title")[Signal Representation]
#pptx.role("footer")[Signal Processing Workshop 2026]
#context [#pptx.role("slide-number", utils.slide-counter.display()) / #utils.last-slide-number]
```

Mark only the number. Text around it (" / 17") then goes to the slide
layout as fixed text, edited once in the slide master, and both PowerPoint
and LibreOffice number the slides themselves. PowerPoint has no field for
the total. If the role holds more than the number, typst-pptx keeps it as
static text in the placeholder (`--slide-total keep`, LibreOffice then
shows it as a plain shape) or drops it (`--slide-total drop`).

Each role wraps its content in two invisible markers:

```typ
#metadata((kind: "title")) <pptx-role-start> ... #metadata("title") <pptx-role-end>
```

## Palette (0.2.0)

```typ
#pptx.palette(dark: navy, light: white, dark2: ink, light2: card,
  accents: (blue, orange, green), link: blue)
```

Becomes PowerPoint's colour scheme (dk1, lt1, dk2, lt2, accent1–6, hlink,
folHlink with `visited-link`); unset slots are inferred from the deck.
Place it once anywhere (the first one counts), e.g. inside a slide
function; at the deck's top level Touying may start an extra slide. It is
`#metadata((dark: "#rrggbb", …, accents: ("#rrggbb", …))) <pptx-palette>`.

## Without the package

A theme can write the markers itself instead of importing the package.
Text between them becomes the placeholder's text; other content (shapes,
pictures) is exported as usual. Typst's own output (PDF, PNG) is unchanged.

Install for local use by copying (or linking) this folder to
`<package path>/local/pptx-hints/0.2.0/`. 0.2.0 adds `palette`; `role` is
unchanged from 0.1.0, so a deck can move to 0.2.0 by changing the import.
