# pptx-hints

Marks the parts of a slide that typst-pptx should export as PowerPoint
placeholders: title, subtitle, body, footer, date, slide number.

```typ
#import "@local/pptx-hints:0.1.0" as pptx

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

A theme can write the markers itself instead of importing the package.
Text between them becomes the placeholder's text; other content (shapes,
pictures) is exported as usual. Typst's own output (PDF, PNG) is unchanged.

Install for local use by copying (or linking) this folder to
`<package path>/local/pptx-hints/0.1.0/`.
