// Content the mapping can't express natively: each item must come out as a
// picture at the exact position (plan §5.8), listed in the report.
#set page(width: 841.89pt, height: 473.56pt, margin: 40pt)
#set text(font: "IBM Plex Sans", size: 18pt)

= Fallbacks

#grid(columns: 3, column-gutter: 40pt, row-gutter: 40pt,
  rotate(-12deg, reflow: true)[Rotated text],
  skew(ax: 15deg, image("assets/gradient.png", width: 120pt)),
  rect(width: 140pt, height: 70pt, fill: gradient.conic(..color.map.rainbow)),
  rect(width: 140pt, height: 70pt, fill: tiling(size: (10pt, 10pt), circle(radius: 3pt, fill: rgb("#2c4f9e")))),
  text(size: 28pt, weight: "bold", fill: gradient.linear(rgb("#c0392b"), rgb("#2c4f9e")))[Gradient text],
  [Plain text stays editable.],
)
