// Touying 0.8 animations (plan §6.2–6.3): every kind of subslide step.
#import "@preview/touying:0.8.0": *
#import themes.simple: *

#set text(font: "IBM Plex Sans")
#show: simple-theme.with(aspect-ratio: "16-9")

= Animations

== Paragraphs with pause

First paragraph, always shown.

#pause

Second paragraph, on the first click.

#pause

Third paragraph, on the second click.

== List with pause

- First item
#pause
- Second item
#pause
- Third item
  - Nested under the third

== Shapes with pause

#grid(
  columns: 3,
  gutter: 1em,
  rect(width: 4cm, height: 2cm, fill: blue.lighten(60%))[One],
  pause,
  circle(radius: 1cm, fill: green.lighten(60%)),
  pause,
  rect(width: 4cm, height: 2cm, radius: 4pt, fill: red.lighten(60%))[Three],
)

== Meanwhile

Left text appears first.

#pause

Then this one.

#meanwhile

Shown from the start, below.

#pause

Shown on the first click, with the second paragraph.

== Uncover and only

#uncover("2-")[Uncovered from the second subslide.]

#uncover("1,3")[On subslides one and three.]

#only(2)[Only on the second, after which the next line moves up.]

Always here.

== Alternatives, same size

#alternatives[First version.][Second version.][Third version.]

Text below stays in place.

== Layout shift

#only(1)[A short line.]
#only(2)[
  A longer block on the second subslide, \
  spanning two lines.
]

This paragraph moves down on the second subslide.

== Alpha cover

#slide(config: config-methods(cover: utils.alpha-changing-cover))[
  Visible from the start.

  #pause

  Dimmed first, then shown in full.

  #pause

  Dimmed until the last click.
]
