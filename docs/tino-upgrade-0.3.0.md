# tino: upgrade typst-pptx 0.1.0 → 0.3.0

For the agent working in tino's repository. tino integrated typst-pptx
0.1.0 following `docs/tino-integration.md` (the CLI contract, owned by
typst-pptx). This file lists what changed up to 0.3.0 and what tino has to
do. The contract at this version:
https://github.com/Steve19802/typst-pptx/blob/v0.3.0/docs/tino-integration.md

Nothing breaks: same command line, exit codes, stderr format and
`report.json` fields as 0.1.0; new fields and flags are additive and their
defaults are what tino should use. The work is the version bump, one new
package to install, and adjusting texts, tests and docs to the new
behaviour.

## 1. What changed for users

| Version | Change |
|---|---|
| 0.1.1 | CFF/OpenType fonts (`.otf`, Typst's built-in Libertinus Serif and New Computer Modern) are converted to TrueType and embedded; before, PowerPoint reported them as missing. |
| 0.2.0 | `--animations auto` (tino's default) exports Touying subslides (`#pause`, `#uncover`, `#only`) as **click animations** on one slide; where the layout shifts, consecutive slides entered with a **Morph** transition. In 0.1.0, `auto` behaved like `slides` (one slide per subslide). A deck now usually has **fewer slides than PDF pages**. |
| 0.3.0 | Content repeated on several slides (theme backgrounds, decorations, headers, footers) goes to shared **slide layouts** (View → Slide Master in PowerPoint). The deck's colours and fonts become the **PowerPoint theme** (Design → Variants recolours the deck; new text boxes use the deck's font). Themes that mark title, subtitle, footer and slide number with the **`pptx-hints`** package get real PowerPoint **placeholders**: titles in Outline view, slide numbers that update when slides are reordered, footer/number managed with Insert → Header & Footer, also in LibreOffice. |

## 2. Tasks

### 2.1 Docker image

- Bump `TYPST_PPTX_VERSION` to `0.3.0` (asset names, URL and archive
  layout unchanged, contract §2). If the download verifies checksums,
  `SHA256SUMS` is in the release as before.
- `typst-pptx --version` prints `typst-pptx 0.3.0 (typst 0.15.1)`: same
  Typst as 0.1.0, so the image check typst ↔ typst-pptx still passes with
  the current typst.
- Install the `pptx-hints` package (§2.2) in the same image.

### 2.2 `pptx-hints` package (new)

A Typst package that marks theme parts as PowerPoint placeholders. It is
invisible in the PDF (plain metadata, layout unchanged), so the same deck
compiles with `typst` and `typst-pptx`. Decks import it as:

```typ
#import "@local/pptx-hints:0.1.0" as pptx
```

A deck or theme that imports it fails to compile, with `typst` and
`typst-pptx` alike, unless the package is in the package path. tino must
therefore install it where both binaries look for packages: the directory
tino passes as `--package-path` (`<pkg dir>/local/pptx-hints/0.1.0/`), or
Typst's data directory if tino relies on that for `@local` packages. It is
not in the release archives; take the three files from the tag:

```dockerfile
# renovate: datasource=github-releases depName=Steve19802/typst-pptx
ARG TYPST_PPTX_VERSION=0.3.0
ARG PPTX_HINTS_VERSION=0.1.0
...
    && mkdir -p "${PKG_DIR}/local/pptx-hints/${PPTX_HINTS_VERSION}" \
    && for f in lib.typ typst.toml README.md; do \
         curl -fsSL "https://raw.githubusercontent.com/Steve19802/typst-pptx/v${TYPST_PPTX_VERSION}/packages/pptx-hints/$f" \
           -o "${PKG_DIR}/local/pptx-hints/${PPTX_HINTS_VERSION}/$f"; \
       done \
```

(`PKG_DIR` = tino's package directory; the package version is the one in
its `typst.toml`, 0.1.0 at typst-pptx 0.3.0.) Usage, for tino's docs and
for themes tino ships:

```typ
#pptx.role("title", self.store.title)
#pptx.role("footer", self.store.footer)
// Only the number: " / N" stays outside, as fixed text on the layout.
#context [#pptx.role("slide-number", utils.slide-counter.display()) / #utils.last-slide-number]
```

Roles: `title`, `subtitle`, `body`, `footer`, `date`, `slide-number`. An
unknown role is a compile error from the package's own `assert`. Decks
without the package export exactly as before (layouts and theme colours
still apply). The package README is in the files above; a complete example
is typst-pptx's `fixtures/themes/signal-wave/template.typ` (search
`pptx.role`).

### 2.3 Command line

Keep the invocation of contract §4. Don't pass the new flags; their
defaults are right for tino:

| Flag | Default | Meaning |
|---|---|---|
| `--layouts auto\|none` | `auto` | `none` = every slide keeps its repeated content (no slide layouts) |
| `--slide-total keep\|drop` | `keep` | only for themes whose `slide-number` role holds "4 / 17": keep the " / 17" as static text in the placeholder, or drop it |

Exposing them in API/MCP/UI is optional and not needed now.

### 2.4 Animation option texts

The `animations` parameter (API query, MCP argument, UI sub-option) is
unchanged, but `auto` now means something different. Update help texts
and docs:

- `auto` (default): subslides become click animations; Morph where the
  layout changes.
- `none`: only the final state of each slide, no animations.
- `slides`: every subslide as its own slide (what `auto` did in 0.1.0).

### 2.5 report.json

Same top-level fields; `report["typst_pptx"]` is `"0.3.0"`. Per slide,
0.2.0 added (contract §6):

```json
{"slide": 4, "page": 4, "pages": [4], "clicks": 0, "transition": null,
 "logical": "4", "items": {"baked": 7, "native": 44}, "notes": true}
```

- `pages`: every Typst page the slide shows (one per click state);
  `clicks`: number of clicks; `transition`: `"morph"` or `null`.
- Don't assume one slide per page: map with `pages`.
- `items` counts the slide's own items. Since 0.3.0, content moved to a
  slide layout is not counted, so the numbers are lower than in 0.1.0 for
  themed decks; don't use them as totals.
- `X-Tino-Pptx-Fallbacks` stays `len(report["baked"])` (signal-wave
  demo: 29 equations, expected).

### 2.6 Fonts

Unchanged contract (§4.1), but check it's in place: the theme's fonts must
reach typst-pptx via `--font-path` or the image, and `unknown font family`
warnings should reach the user. Since 0.1.1 a missing font no longer makes
PowerPoint report Libertinus as missing (it is embedded), so the fallback
look is the only symptom left.

### 2.7 Tests

- Fake binary: version line `typst-pptx 0.3.0 (typst 0.15.1)`; the
  `--report` JSON it writes can include `pages`, `clicks`, `transition`.
- Integration test with the real binary (when present): a deck that
  imports `@local/pptx-hints:0.1.0` compiles with exit 0, proving the
  package is installed; optionally a Touying deck with `#pause` gives fewer
  slides than pages (`len(report["slides"])` < pages) with `clicks` > 0.
- A test that `typst compile` (PDF) of the same deck also works, since
  both binaries need the package.

### 2.8 Docs

- Editor/API/MCP pages: the new meaning of `animations` (§2.4); exported
  decks have slide layouts and the deck's theme colours/fonts.
- Theme authors: `pptx-hints` (§2.2), mark only the slide number; how to
  import it in tino.
- MCP `instructions.py`: unchanged line about `report.baked`; optionally
  mention `slides[].clicks`.
- ADR 11: a note that 0.3.0 adds slide layouts and the `pptx-hints`
  package installed with the binary.
- Deployment: the package install and its version pin.

## 3. Check after the upgrade

1. Image: `typst-pptx --version` → `typst-pptx 0.3.0 (typst 0.15.1)`;
   `ls <pkg dir>/local/pptx-hints/0.1.0` → `lib.typ typst.toml README.md`.
2. A deck with `#import "@local/pptx-hints:0.1.0" as pptx` and
   `#pptx.role("title")[Hello]` compiles to PDF and to PPTX.
3. Export a Touying deck with `#pause` from the UI: PowerPoint shows one
   slide that builds on click (slide show), not one slide per step.
4. A themed deck in PowerPoint: View → Slide Master shows the shared
   layouts with the theme's background; a theme using `pptx-hints` shows
   its titles in View → Outline.

Requests for further flags, messages or report fields go to the
typst-pptx repository through the owner (contract §9).
