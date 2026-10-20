// Text placement fixture: sizes, line spacings, mixed sizes and alignments.
// Checked line by line with tools/validate/text_positions.py, against
// LibreOffice in CI and against PowerPoint's PDF export at milestones.
#set page(width: 841.89pt, height: 473.56pt, margin: 30pt)
#set text(font: "IBM Plex Sans", size: 14pt)

#let sample = [Hamburgefonstiv quick brown fox]

= Single lines
#for size in (10pt, 14pt, 20pt, 28pt, 40pt, 60pt) [
  #text(size: size, sample) \
]

#pagebreak()
= Line spacing
#grid(columns: 3, column-gutter: 24pt, row-gutter: 18pt,
  ..for leading in (0.3em, 0.65em, 1em, 1.5em, 2em, 3em) {
    (block(width: 240pt)[
      #set par(leading: leading)
      Leading #repr(leading): three lines of wrapped text so the line pitch is
      visible across the whole paragraph.
    ],)
  }
)

#pagebreak()
= Sizes in paragraphs
#for size in (10pt, 16pt, 24pt, 36pt) [
  #block(width: 600pt)[
    #set text(size: size)
    #set par(leading: 0.5em)
    At #repr(size) this paragraph wraps onto a second line of text in the box.
  ]
]

#pagebreak()
= Mixed sizes
#block(width: 500pt)[
  A paragraph whose second line has a #text(size: 30pt)[larger] word and a
  #text(size: 9pt)[smaller] one, with ordinary text around them on both lines.
]
#stack(spacing: 4pt, text(size: 36pt, weight: "bold")[1024], text(size: 12pt)[FFT points])

#pagebreak()
= Monospace and alignment
#set text(font: "IBM Plex Mono")
#block(width: 400pt)[
  Monospaced text, set in IBM Plex Mono, wrapping over a couple of lines.
]
#set text(font: "IBM Plex Sans")
#align(center)[A centred line] \
#align(right)[A right-aligned line]
