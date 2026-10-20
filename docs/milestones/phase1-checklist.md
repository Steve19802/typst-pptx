# Phase 1 milestone check (Windows, PowerPoint)

About 30 minutes. Text positions and the overall look are measured
automatically from PowerPoint's PDF export (`make milestone-report`); the
manual part covers what a PDF can't show.

This round tests the embedded fonts, so IBM Plex must **not** be installed.

## 1. Setup

1. If IBM Plex is still installed from Phase 0: Settings → Personalization →
   Fonts, search "IBM Plex", uninstall every face (Sans, Mono, Medium,
   SemiBold, …). Close PowerPoint completely afterwards.
2. Leave `fonts/` alone; it's only needed for the optional step at the
   end.
3. Write the PowerPoint version (File → Account → About PowerPoint) into
   `powerpoint-windows-version.txt` next to this checklist.

## 2. PDF export (the measured part)

For every `.pptx` in `decks/`: open it in the **PowerPoint desktop app**
(not PowerPoint for the web or a browser), File → Export → Create
PDF/XPS, save it next to this checklist as `<name>-windows.pdf` (e.g.
`decks-features-windows.pdf`). Default options are fine. Browser "Save as
PDF" rounds the page to whole pixels, which shifts every measured line.

Then copy the whole folder back into `out/milestone/` of the repository.

## 3. Manual checks

Note pass / fail and a short remark per item in `results.md` next to this
checklist; add screenshots for failures. The Typst PDFs in `decks/` show
what each slide should look like.

Fonts (plan §5.1):

- [ ] 3.1 Every deck opens without a repair prompt and without a message
      about fonts.
- [ ] 3.2 `decks-features` slide 1: the text is IBM Plex (compare with
      `decks/decks-features.pdf`), not a substitute like Calibri or Arial.
      Click into "SemiBold face": the font box shows "IBM Plex Sans SmBld".
- [ ] 3.3 `signal-wave-demo` slide 4: click into a paragraph and type a few
      characters: editing works and the new text is IBM Plex Sans.
- [ ] 3.4 `decks-text-metrics` slide 5: the monospaced text is IBM Plex Mono.
- [ ] 3.5 Save `decks-features` under a new name (File → Save As), close
      PowerPoint, reopen the copy: the text is still IBM Plex.

Shapes, links, rotation:

- [ ] 3.6 `decks-features` slide 5: the dotted line (second line, round
      dots) shows nearly touching dots as in the Typst PDF, not dots with
      wide gaps. Note which.
- [ ] 3.7 `decks-features` slide 6: the badge (middle picture) is a group of
      shapes: click it, then click a part of it; the parts can be selected
      and recoloured.
- [ ] 3.8 `decks-features` slide 8, in the slide show (F5): "Typst website"
      and the picture open typst.app in the browser; "the shapes slide"
      jumps to slide 4. In the editor, the link text keeps its dark colour
      and has no underline (note the colour if not).
- [ ] 3.9 `signal-wave-demo` slide 2, in the slide show: clicking
      "Implementation" jumps to slide 7 (the "Implementation" section
      slide).
- [ ] 3.10 `decks-features` slide 9: every item is rotated like in the Typst
      PDF; click the rotated paragraph and type: it stays rotated and wraps
      as before. The rotation handle shows the angle (Format → Size).

Fallbacks:

- [ ] 3.11 `decks-fallbacks`: the mirrored text, the skewed image and the
      gradient/tiling items look like the Typst PDF (they are pictures).
- [ ] 3.12 Anything else that looks wrong.

Optional, only if 3.2 failed: install the fonts in `fonts/`, restart
PowerPoint and note whether `decks-features` slide 1 then looks right.
