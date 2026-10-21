// Tables (plan §5.6): native a:tbl where the look carries over, shapes and
// text boxes otherwise (each listed in the report).
#set page(width: 841.89pt, height: 473.56pt, margin: 40pt)
#set text(font: "IBM Plex Sans", size: 16pt)

= Default strokes, alignment

#table(
  columns: (1fr, auto, 1fr),
  align: (left, center, right),
  [Left aligned], [Centre], [Right aligned],
  [A longer cell whose text wraps onto a second line], [], [42.0],
  [#link("https://typst.app")[Linked cell]], [x], [−3.5],
)

#pagebreak()
= Merged cells, fills and partial strokes

#table(
  columns: 4,
  stroke: none,
  fill: (x, y) => if y == 0 { rgb("#2c4f9e") } else if calc.even(y) { rgb("#edf2f9") },
  table.hline(y: 1, stroke: 1.5pt + rgb("#2c4f9e")),
  table.vline(x: 1, stroke: 0.5pt + rgb("#8a9bb8")),
  table.header(
    table.cell(colspan: 2, text(fill: white, weight: "bold")[Spans two columns]),
    text(fill: white, weight: "bold")[Three],
    text(fill: white, weight: "bold")[Four],
  ),
  table.cell(rowspan: 2, align: horizon)[Spans two rows], [b], [c], [d],
  [f], [g], [h],
  [i], [j], [k], [l],
)

#pagebreak()
= Fallbacks

#grid(columns: 3, column-gutter: 30pt,
  table(columns: 2, [Image], image("assets/gradient.png", width: 60pt)),
  table(columns: 2, column-gutter: 6pt, [Gutter], [cells]),
  table(columns: 1, [First paragraph.

  Second paragraph.]),
)
