// Touying 0.8 simple theme: subslides and speaker notes.
#import "@preview/touying:0.8.0": *
#import themes.simple: *

#set text(font: "IBM Plex Sans")
#show: simple-theme.with(aspect-ratio: "16-9")

= Title slide

#speaker-note[Notes on the title slide.]

== Reveal

First point

#pause

Second point

#pause

Third point

#speaker-note[First paragraph of the notes.

Second paragraph of the notes.]

== Alternatives

#only(1)[Only on the first subslide]
#only(2)[Only on the second subslide]
