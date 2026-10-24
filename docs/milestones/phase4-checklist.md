# Phase 4 milestone check (Windows, PowerPoint)

About 30 minutes. Phase 4 turns Touying subslides into click animations
(plan §6.2–6.3). The look of every subslide is still measured from
PowerPoint's PDF export (`decks/`, every subslide as its own slide); the
animations themselves can only be checked in the slideshow (`animated/`).

## 1. Setup

1. Fonts are embedded; nothing to install. Make sure Libertinus Serif and
   New Computer Modern are **not** installed (Settings → Fonts): an
   installed family takes precedence over the embedded one.
2. Write the PowerPoint version (File → Account → About PowerPoint) into
   `powerpoint-windows-version.txt` next to this checklist. If PowerPoint
   2021 is available too, note its version as well: section 4 is for it.

## 2. PDF export (the measured part)

For every `.pptx` in `decks/`: open it in the **PowerPoint desktop app**,
File → Export → Create PDF/XPS, save it next to this checklist as
`<name>-windows.pdf` (exactly that name, e.g. `decks-animations-windows.pdf`).

Then copy the whole folder back into `out/milestone/` of the repository.

## 3. Slideshow checks (PowerPoint 365)

Open `animated/decks-animations.pptx` and play it from the start (F5).
Each click (or →) should look like the next page of
`decks/decks-animations.pdf`; the page numbers are given below. Note pass /
fail and a short remark per item in `results.md`; a phone video of a
failure helps.

- [ ] 4.1 The file opens without a repair prompt. In normal view, the
      Animations tab → Animation Pane lists effects on slides 2–5, 9 and
      12 only.
- [ ] 4.2 Slide 2 "Paragraphs with pause": starts with the first paragraph
      only (PDF page 2); click → second paragraph (page 3); click → third
      (page 4); next click goes to slide 3.
- [ ] 4.3 Slide 3 "List with pause": first item only (page 5); click →
      second item (page 6); click → third item **and** its nested item
      together (page 7). Out of the slideshow: the list is **one** text
      box, and the Animation Pane shows its paragraphs as separate
      entries.
- [ ] 4.4 Slide 4 "Shapes with pause": blue box with "One" (page 8);
      click → green circle (page 9); click → red box with "Three" (page
      10).
- [ ] 4.5 Slide 5 "Meanwhile": first and third lines (page 11); one click
      shows the second and the fourth together (page 12).
- [ ] 4.6 Slides 6–8 "Uncover and only": three slides (pages 13, 14, 15),
      each entered with **Morph**: the title and the footer stay still,
      "Always here." glides down and back up, the other lines fade.
- [ ] 4.7 Slide 9 "Alternatives": "First version." (page 16); click →
      "Second version." in the same place, the first gone (page 17); click
      → "Third version." (page 18). Nothing else moves.
- [ ] 4.8 Slides 10–11 "Layout shift": slide 11 is entered with Morph,
      "This paragraph moves down…" glides down (pages 19, 20).
- [ ] 4.9 Slide 12 "Alpha cover": first line black, the other two grey
      (page 21); click → second line black (page 22); click → third line
      black (page 23). No line is ever doubled or offset.
- [ ] 4.10 Going **back** (←) through slides 2–12 undoes each click in
      order.
- [ ] 4.11 `animated/decks-touying-basic.pptx`: slide 2 reveals "Second
      point" and "Third point" on two clicks; slide 3 swaps the two
      `#only` lines on one click. Speaker notes (Presenter View) are on
      slides 1 and 2.
- [ ] 4.12 Anything else that looks wrong.

## 4. PowerPoint 2021 (if available)

- [ ] 4.13 Repeat 4.2, 4.3, 4.6 and 4.9 in PowerPoint 2021.

## 5. Carried over

- [ ] 4.14 `decks/signal-wave-demo.pptx` and `decks/decks-features.pptx`
      open without missing-font prompts, and bold text is bold.
