# Phase 0 milestone check (Windows, PowerPoint)

About 20 minutes. Text positions and the overall look are measured
automatically from PowerPoint's PDF export (`make milestone-report`); the
manual part covers what a PDF can't show.

## 1. Setup

1. Fonts are not embedded yet (Phase 1). Install every file in `fonts/`
   (select all → right click → Install), then start PowerPoint.
2. Write the PowerPoint version (File → Account → About PowerPoint) into
   `powerpoint-windows-version.txt` next to this checklist.

## 2. PDF export (the measured part)

For every `.pptx` in `decks/`: open it, File → Export → Create PDF/XPS,
save it next to this checklist as `<name>-windows.pdf` (e.g.
`decks-features-windows.pdf`). Default options are fine.

Then copy the whole folder back into `out/milestone/` of the repository.

## 3. Manual checks

Note pass / fail and a short remark per item in `results.md` next to this
checklist; add screenshots for failures.

- [ ] 3.1 Every deck opens without a repair prompt.
- [ ] 3.2 `decks-features` slide 1: click into "SemiBold face": the font box
      shows "IBM Plex Sans SmBld".
- [ ] 3.3 `decks-features` slide 4: the orange rounded rectangle shows a
      yellow corner-radius handle when selected.
- [ ] 3.4 `decks-features` slide 6: right click the block equation → View
      Alt Text shows the formula source.
- [ ] 3.5 `signal-wave-demo` slide 4: click into a paragraph and type: the
      text is editable and stays IBM Plex Sans; lines don't re-wrap.
- [ ] 3.6 View → Notes: `signal-wave-demo` slides 3, 7 and 11 have no notes,
      all others do; `decks-touying-basic` slide 2 has two notes paragraphs.
- [ ] 3.7 Anything else that looks wrong.
