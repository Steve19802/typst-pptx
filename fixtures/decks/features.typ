// Plain Typst feature deck (no packages): one mapping topic per page.
// Fonts: IBM Plex from fixtures/fonts plus Typst's embedded math font.
#set document(title: "Feature deck", author: "typst-pptx")
#set page(width: 841.89pt, height: 473.56pt, margin: 40pt, fill: rgb("#fbfbf8"))
#set text(font: "IBM Plex Sans", size: 18pt, fill: rgb("#1a2744"))

= Text styles

Regular, *bold*, _italic_, *_bold italic_* and `mono` in one paragraph.
#text(fill: rgb("#c0392b"))[Red], #text(fill: rgb("#1a2744").transparentize(50%))[half transparent],
#text(size: 12pt)[small] and #text(size: 28pt)[large].

#text(tracking: 3pt, size: 12pt)[TRACKED CAPS]

// Upstream Plex names these faces by their legacy family (name ID 1).
#text(font: "IBM Plex Sans Medm")[Medium face] and #text(font: "IBM Plex Sans SmBld")[SemiBold face].

#pagebreak()
= Paragraphs

#block(width: 330pt)[
  A left-aligned paragraph that wraps over several lines, so the exporter
  has to keep Typst's line breaks and spacing exactly.
]
#align(center, block(width: 330pt)[
  #set align(center)
  A centred paragraph that also wraps over more than one line of text.
])
#align(right, block(width: 330pt)[
  #set align(right)
  A right-aligned paragraph with two lines of text in it.
])
#block(width: 330pt)[
  #set par(leading: 1.2em)
  Wider leading changes the exact line spacing of this paragraph's lines.
]
#block(width: 330pt)[
  #set par(justify: true)
  A justified paragraph stretches the spaces of every line but the last one,
  so the office's final figures fit flush against both edges of the block.
]

#pagebreak()
= Lists

- First item
- Second item with *strong* text
  - Nested item
+ Numbered one
+ Numbered two

#pagebreak()
= Shapes <shapes>

#grid(columns: 5, column-gutter: 20pt, row-gutter: 20pt,
  rect(width: 100pt, height: 60pt, fill: rgb("#2c4f9e")),
  rect(width: 100pt, height: 60pt, radius: 12pt, fill: rgb("#f2a65a"), stroke: 2pt + rgb("#1a2744")),
  ellipse(width: 100pt, height: 60pt, fill: rgb("#7fb069")),
  rect(width: 100pt, height: 60pt, radius: (top: 12pt), fill: rgb("#9b59b6")),
  polygon(fill: rgb("#e74c3c"), (0pt, 60pt), (50pt, 0pt), (100pt, 60pt)),
  line(length: 100pt, stroke: 3pt + rgb("#1a2744")),
  line(start: (0pt, 60pt), end: (100pt, 0pt), stroke: 1pt + rgb("#c0392b")),
  rect(width: 100pt, height: 60pt, stroke: (bottom: 2pt + rgb("#2c4f9e"))),
  curve(fill: rgb("#3498db"), curve.move((0pt, 30pt)), curve.cubic((30pt, -20pt), (70pt, 80pt), (100pt, 30pt)), curve.line((100pt, 60pt)), curve.line((0pt, 60pt)), curve.close()),
  circle(radius: 30pt, fill: rgb("#1a2744").transparentize(60%)),
)

#pagebreak()
= Fills and strokes

#grid(columns: 4, column-gutter: 20pt, row-gutter: 20pt,
  rect(width: 160pt, height: 60pt, fill: gradient.linear(rgb("#2c4f9e"), rgb("#f2a65a"))),
  rect(width: 160pt, height: 60pt, fill: gradient.linear(rgb("#2c4f9e"), rgb("#f2a65a"), angle: 45deg)),
  rect(width: 160pt, height: 60pt, fill: gradient.linear(rgb("#2c4f9e"), rgb("#f2a65a"), angle: 90deg)),
  rect(width: 160pt, height: 60pt, radius: 10pt, fill: gradient.linear(red, blue, angle: 135deg)),
  rect(width: 160pt, height: 60pt, fill: gradient.linear(..color.map.viridis)),
  ellipse(width: 160pt, height: 60pt, fill: gradient.radial(rgb("#f2a65a"), rgb("#2c4f9e"))),
  rect(width: 60pt, height: 60pt, fill: gradient.radial(white, rgb("#2c4f9e"), radius: 30%)),
  rect(width: 160pt, height: 60pt, fill: gradient.linear(rgb("#ff000080"), rgb("#0000ff"))),
  line(length: 160pt, stroke: (paint: rgb("#1a2744"), thickness: 3pt, dash: "dashed")),
  line(length: 160pt, stroke: (paint: rgb("#1a2744"), thickness: 3pt, dash: "dotted", cap: "round")),
  line(length: 160pt, stroke: (paint: rgb("#1a2744"), thickness: 2pt, dash: "dash-dotted")),
  polygon(stroke: (paint: rgb("#c0392b"), thickness: 6pt, join: "round"), (0pt, 50pt), (60pt, 0pt), (120pt, 50pt)),
)

#pagebreak()
= Pictures

#grid(columns: 3, column-gutter: 30pt,
  image("assets/gradient.png", width: 150pt, alt: "A colour gradient"),
  image("assets/badge.svg", width: 150pt),
  box(clip: true, width: 150pt, height: 60pt, image("assets/gradient.png", width: 150pt, height: 60pt, fit: "cover")),
)

#pagebreak()
= Math

The rate $f_s = 2 f_max$ appears inline in this sentence.

$ X[k] = sum_(n=0)^(N-1) x[n] dot e^(-j 2 pi k n \/ N) $

#pagebreak()
= Links

The #link("https://typst.app")[Typst website] is an external link, and
#link(<shapes>)[the shapes slide] jumps inside the deck.

#link("https://typst.app/docs", image("assets/gradient.png", width: 120pt))

#pagebreak()
= Rotation

#grid(columns: 4, column-gutter: 40pt, row-gutter: 30pt, align: center + horizon,
  rotate(-15deg, reflow: true, block(width: 160pt)[A rotated paragraph that wraps over two lines.]),
  rotate(30deg, reflow: true, rect(width: 120pt, height: 50pt, radius: 8pt, fill: gradient.linear(rgb("#2c4f9e"), rgb("#f2a65a")))),
  rotate(45deg, reflow: true, line(length: 100pt, stroke: (paint: rgb("#c0392b"), thickness: 3pt, dash: "dashed"))),
  rotate(-20deg, reflow: true, image("assets/gradient.png", width: 120pt)),
  rotate(180deg, reflow: true)[Upside down],
  rotate(90deg, reflow: true, polygon(fill: rgb("#7fb069"), (0pt, 60pt), (50pt, 0pt), (100pt, 60pt))),
  rotate(10deg, reflow: true, rect(width: 140pt, inset: 8pt, fill: rgb("#e8eef8"), stroke: 1pt + rgb("#2c4f9e"))[Text in a rotated box]),
)
