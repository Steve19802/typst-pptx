# Phase 0 milestone check (Windows, PowerPoint)

About 30 minutes. Record the results in a GitHub issue titled
"Milestone check: Phase 0", one line per item (pass / fail + note), and
attach screenshots of every failure.

Note your PowerPoint version (File → Account → About PowerPoint).

## 0. Setup

Fonts are not embedded yet (Phase 1). Install every file in `fonts/`
(select all → right click → Install), then start PowerPoint.

## 1. Opening

For each file in `decks/` (`decks-features`, `decks-touying-basic`, `signal-wave-demo`,
`calibration`):

- [ ] 1.1 Opens without a repair prompt.
- [ ] 1.2 Slide count matches the PDF next to it (calibration: 60 slides, no PDF).

## 2. Text baseline (most important)

Open `calibration.pptx`. Each slide has two lines of "Hxg" and two thin red
lines; the red lines are where LibreOffice puts the baselines.

- [ ] 2.1 On slides 1–15 (IBM Plex Sans), the bottom of "H" sits on the red
      lines. If not: zoom to 400 % on slides 1, 2 and 7 and screenshot.
- [ ] 2.2 Same for slides 16–30 (Libertinus Serif), 31–45 (New Computer
      Modern), 46–60 (DejaVu Sans Mono). If one font is off and the others
      are fine, note which.

## 3. Look against the PDF

Compare each slide of `decks-features.pptx` and `signal-wave-demo.pptx` with the
PDF at the same zoom (side by side, or toggle between windows).

- [ ] 3.1 Text lines start and break at the same places as in the PDF.
- [ ] 3.2 No line wraps differently or runs past its box.
- [ ] 3.3 features slide 1: "TRACKED CAPS" is letter-spaced like the PDF.
- [ ] 3.4 features slide 1: "Medium face" and "SemiBold face" show the medium
      and semibold weights (not regular/bold). Click into the text: the font
      box shows "IBM Plex Sans Medm" / "IBM Plex Sans SmBld".
- [ ] 3.5 features slide 4: rounded rectangle, ellipse, triangle, wave shape
      and circle look like the PDF; the rounded rectangle has a yellow
      corner-radius handle when selected.
- [ ] 3.6 features slide 5 and demo backgrounds: pictures are cropped like
      the PDF (third picture on slide 5; the bars on section slides).
- [ ] 3.7 Equations (features slide 6, demo slides 4–6, 12) look identical;
      right click → Edit Alt Text shows the formula source.
- [ ] 3.8 Note any slide that looks different and how.

## 4. Editing and notes

- [ ] 4.1 Click into any paragraph on demo slide 4 and type: text is editable,
      the font stays IBM Plex Sans.
- [ ] 4.2 View → Notes: demo slides 1, 2, 4–6, 8–10 and 12–17 have speaker
      notes; slides 3, 7 and 11 have none.
- [ ] 4.3 `decks-touying-basic.pptx`: slide 2 has two notes paragraphs; slides 3
      and 4 (the later reveal steps) have no notes.
