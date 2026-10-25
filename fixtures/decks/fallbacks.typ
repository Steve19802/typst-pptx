// Content the mapping can't express natively: each item must come out as a
// picture at the exact position (plan §5.8), listed in the report.
#set page(width: 841.89pt, height: 473.56pt, margin: 40pt)
#set text(font: "IBM Plex Sans", size: 18pt)

= Fallbacks

#grid(columns: 3, column-gutter: 40pt, row-gutter: 40pt,
  scale(x: -100%, reflow: true)[Mirrored text],
  skew(ax: 15deg, image("assets/gradient.png", width: 120pt)),
  rect(width: 140pt, height: 70pt, fill: gradient.conic(..color.map.rainbow)),
  rect(width: 140pt, height: 70pt, fill: tiling(size: (10pt, 10pt), circle(radius: 3pt, fill: rgb("#2c4f9e")))),
  text(size: 28pt, weight: "bold", fill: gradient.linear(rgb("#c0392b"), rgb("#2c4f9e")))[Gradient text],
  [Plain text stays editable.],
  rect(width: 140pt, height: 70pt, fill: gradient.radial(white, rgb("#2c4f9e"), center: (20%, 30%))),
  box(width: 200pt, height: 40pt, fill: gradient.linear(red, blue, relative: "parent"))[],
)

#pagebreak()

= Clipped content

// A clip cuts only what crosses it: the blue band is cut natively, the
// circle, the line and the text running out are pictures cut to the box,
// the dot outside is dropped. In the rounded card only the header band
// crosses the corners; the card fitting its clip stays native.
#grid(columns: 3, column-gutter: 30pt,
  box(width: 200pt, height: 110pt, clip: true, stroke: 0.5pt, {
    place(dx: -20pt, dy: 15pt, rect(width: 260pt, height: 30pt, fill: rgb("#2c4f9e")))
    place(dx: 150pt, dy: 70pt, circle(radius: 40pt, fill: rgb("#c0392b")))
    place(curve(stroke: 1.5pt, curve.move((0pt, 100pt)), curve.line((100pt, 55pt)), curve.line((230pt, 80pt))))
    place(dx: 110pt, dy: 52pt)[Text running out]
    place(dx: 230pt, dy: 10pt, circle(radius: 5pt))
  }),
  box(width: 200pt, height: 110pt, radius: 14pt, clip: true, stroke: 0.5pt, {
    place(rect(width: 200pt, height: 28pt, fill: rgb("#2c4f9e")))
    place(dx: 12pt, dy: 45pt)[Card body]
  }),
  box(width: 180pt, height: 110pt, radius: 14pt, clip: true, inset: 12pt, fill: luma(235))[Fits its clip],
)
