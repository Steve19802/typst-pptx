# tino: typst-pptx 0.4.0

Follow `docs/tino-upgrade-0.3.0.md` first (or together); this adds what
0.4.0 changes. No CLI change, no new flags.

## Changes

- Content crossing a `clip: true` box (charts, cropped decorations) is cut
  or exported as a picture cut to the box; up to 0.3.0 it showed in full.
  New `report.json` `baked` reason `"clipped content"` and the stderr
  warning `exported as a picture: clipped content` (at the element's
  position, like the other `exported as a picture` warnings). It counts
  in `X-Tino-Pptx-Fallbacks` like any baked item.
- Uniformly scaled content (Touying's `fit-to-width`, `scale(80%)`) is
  native text, shapes and pictures instead of baked: fewer baked items.
- `pptx-hints` **0.2.0** adds `palette(...)`, which sets the PowerPoint
  colour scheme. `role` is unchanged.

## Tasks

1. `TYPST_PPTX_VERSION=0.4.0` (assets and layout unchanged; still
   `typst 0.15.1`).
2. Install `pptx-hints` 0.2.0 **and keep 0.1.0**: decks import a fixed
   version, and published versions never change. Each version comes from
   the tag that introduced it:

   ```dockerfile
   && for v in "0.1.0:v0.3.0" "0.2.0:v0.4.0"; do \
        ver="${v%%:*}"; tag="${v##*:}"; \
        mkdir -p "${PKG_DIR}/local/pptx-hints/${ver}"; \
        for f in lib.typ typst.toml README.md; do \
          curl -fsSL "https://raw.githubusercontent.com/Steve19802/typst-pptx/${tag}/packages/pptx-hints/$f" \
            -o "${PKG_DIR}/local/pptx-hints/${ver}/$f"; \
        done; \
      done \
   ```

3. Tests: the fake binary's version line is `typst-pptx 0.4.0 (typst
   0.15.1)`; the diagnostics parser needs nothing new. An integration test
   can import `@local/pptx-hints:0.2.0` and call
   `#pptx.palette(accents: (blue,))` inside a slide.
4. Docs for theme authors: link
   https://github.com/Steve19802/typst-pptx/blob/v0.4.0/docs/theme-authoring.md
   (rules for themes that export well, including `pptx.palette`).
