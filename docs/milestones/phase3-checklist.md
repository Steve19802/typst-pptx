# Phase 3 milestone check (Windows, PowerPoint)

About 25 minutes. Text positions (now including table cells and list
items) and the overall look are measured automatically from PowerPoint's
PDF export (`make milestone-report`); the manual part covers what a PDF
can't show.

## 1. Setup

1. As in Phase 1: IBM Plex need not be installed (fonts are embedded).
2. Write the PowerPoint version (File → Account → About PowerPoint) into
   `powerpoint-windows-version.txt` next to this checklist.

## 2. PDF export (the measured part)

For every `.pptx` in `decks/`: open it in the **PowerPoint desktop app**,
File → Export → Create PDF/XPS, save it next to this checklist as
`<name>-windows.pdf` (exactly that name, e.g. `decks-tables-windows.pdf`).

Then copy the whole folder back into `out/milestone/` of the repository.

## 3. Manual checks

Note pass / fail and a short remark per item in `results.md` next to this
checklist; add screenshots for failures.

Tables (plan §5.6):

- [ ] 3.1 Every deck opens without a repair prompt.
- [ ] 3.2 `signal-wave-demo` slide 10: click the table. It is one table
      (the Table Design and Layout tabs appear), not separate shapes.
- [ ] 3.3 Same table, Layout tab (or Format → Size): its height is
      **7.77 cm (3.06")** and width 26.88 cm. A larger height means
      PowerPoint grew a row; note the value. Same for slide 9: height
      5.81 cm (2.29").
- [ ] 3.4 `decks-tables` slide 1: height 3.00 cm (1.18"); the long cell
      still wraps after "second" as in the Typst PDF. Slide 2: height
      2.99 cm (1.18"); "Spans two columns" and "Spans two rows" are merged
      cells (one selection each).
- [ ] 3.5 `signal-wave-demo` slide 10: click into "Bessel" and type
      " filter": the text stays IBM Plex and the row keeps its height
      while the text fits. Undo.
- [ ] 3.6 `decks-tables` slide 3: the three small tables are shapes and
      text boxes (expected, they can't be PowerPoint tables).

Lists:

- [ ] 3.7 `decks-features` slide 3: the bullet list is one text box with
      real bullets: click at the end of "Second item with strong text",
      press Enter and type: a new bullet appears at the same position.
      Undo.
- [ ] 3.8 Same slide, numbered list: Enter at the end of "Numbered two"
      gives "3.". Undo.
- [ ] 3.9 Same slide: "Nested item" is indented one level (Home →
      Decrease List Level moves it to the first level). Undo.
- [ ] 3.10 `signal-wave-demo` slides 5, 6, 9, 12, 13, 15: the bullet
      lists look like the Typst PDF (bullet glyphs, indents, spacing).
- [ ] 3.11 Anything else that looks wrong.
