# Phase 5 milestone check (Windows, PowerPoint)

About 25 minutes. Phase 5 adds slide layouts for repeated content, the
deck's colours and fonts as the PowerPoint theme, and placeholders for
themes that use `pptx-hints` (plan §7). Text positions are still measured
from PowerPoint's PDF export; the rest is checked in PowerPoint's UI.

## 1. Setup

1. Fonts are embedded; nothing to install. Libertinus Serif and New
   Computer Modern must **not** be installed (Settings → Fonts).
2. Write the PowerPoint version (File → Account → About PowerPoint) into
   `powerpoint-windows-version.txt` next to this checklist.

## 2. PDF export (the measured part)

For every `.pptx` in `decks/`: open it in the **PowerPoint desktop app**,
File → Export → Create PDF/XPS, save it next to this checklist as
`<name>-windows.pdf`. Then copy the folder back into `out/milestone/`.

## 3. Checks in PowerPoint

Use `decks/signal-wave-demo.pptx` unless noted. Note pass / fail and a
remark per item in `results.md`; screenshots for failures.

Layouts:

- [ ] 5.1 The file opens without a repair prompt or missing-font message.
- [ ] 5.2 View → Slide Master: besides "Blank" there are three layouts
      ("Layout 1–3"). Layout 3 shows the grid, sine waves, header line and
      footer decorations; Layout 2 the section background; Layout 1 the
      title/ending frame. Close Master View.
- [ ] 5.3 Slide 4: click the background grid or the footer wave icon:
      they can't be selected on the slide (they belong to the layout).
      Home → Layout shows Layout 3 selected.
- [ ] 5.4 Slides 2 (outline) and 16 (focus) use "Blank" and look as
      before.

Placeholders and the slide number:

- [ ] 5.5 Slide 4: click "Signal Representation": the selection is a
      **title placeholder** (Home → Reset doesn't move or restyle it;
      Outline View, View → Outline, lists the slide titles).
- [ ] 5.6 Slide 1: the title and "Foundations of Modern DSP Systems" are
      title and subtitle placeholders.
- [ ] 5.7 Drag slide 5 before slide 4 in the thumbnail pane: the bottom
      right numbers update ("4 / 17" now on "Core Transforms"). Undo.
- [ ] 5.8 Insert → Header & Footer: "Slide number" and "Footer" are
      ticked for the content slides (they are real placeholders).

Theme:

- [ ] 5.9 Design → Variants → Colors (or the colour picker of any shape):
      the theme colours are the deck's (blue 2E5090, orange E85D3A, green
      3A8C6E first), not Office's.
- [ ] 5.10 Design → Variants → Colors → pick another palette (e.g.
      "Blue II"): the cards, header text and lines recolour. Undo.
- [ ] 5.11 Insert → Text Box on any slide and type: the font is IBM Plex
      Sans, not Calibri/Arial. Undo.

Regressions:

- [ ] 5.12 `animated/decks-animations.pptx` still plays as in Phase 4
      (spot-check slides 3, 6–8, 12).
- [ ] 5.13 Anything else that looks wrong.
