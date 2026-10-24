# pptx-hints

Marks the parts of a slide that typst-pptx should export as PowerPoint
placeholders: title, subtitle, body, footer, date, slide number.

```typ
#import "@local/pptx-hints:0.1.0" as pptx

#pptx.role("title")[Signal Representation]
#pptx.role("footer")[Signal Processing Workshop 2026]
#pptx.role("slide-number")[#context utils.slide-counter.display()]
```

Each role wraps its content in two invisible markers:

```typ
#metadata((kind: "title")) <pptx-role-start> ... #metadata("title") <pptx-role-end>
```

A theme can write the markers itself instead of importing the package.
Text between them becomes the placeholder's text; other content (shapes,
pictures) is exported as usual. Typst's own output (PDF, PNG) is unchanged.

Install for local use by copying (or linking) this folder to
`<package path>/local/pptx-hints/0.1.0/`.
