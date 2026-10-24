# typst-pptx in tino: integration handover

For the agent/developer integrating typst-pptx into tino (plan
`touying-pptx-exporter.md` §8 and §10). This repository owns the CLI
contract below; changes to it are made here first and announced with a new
release. Every sample was produced with the released `v0.1.0` binary unless
marked 0.1.1.

Changes since 0.1.0:

- **0.1.1:** CFF-flavoured OpenType fonts (`.otf`, including Typst's
  built-in Libertinus Serif and New Computer Modern) are converted to
  TrueType and embedded; before, PowerPoint reported them as missing. Lists
  whose line pitch isn't a whole number of points (e.g. in Libertinus)
  stay native. No CLI or `report.json` change. See §4.1 for what tino must
  do about fonts.
- **0.2.0 (Phase 4):** `--animations auto` (the default, what
  tino passes) turns Touying subslides into click animations: one slide per
  logical slide where the layout stays put, consecutive slides entered
  with Morph where it shifts. `report.json` slide entries gain `pages`,
  `clicks` and `transition` (§6); an item on several subslides of one slide
  is reported once. No CLI change.
- **0.3.0 (in progress, Phase 5):** content repeated on several slides
  (theme backgrounds, headers, footers) goes to shared slide layouts; new
  flag `--layouts auto|none` (default `auto`, nothing for tino to change).
  The PowerPoint theme gets the deck's colours and fonts; colours equal to
  a theme colour are written as theme references (recolourable).

## 1. What v0.1.0 does

`typst-pptx 0.1.0 (typst 0.15.1)`, Touying ≥ 0.8.0, < 0.9.

- Exports each Typst page as an editable PowerPoint slide with Typst's
  look: text boxes with Typst's line breaks, shapes, gradients, pictures,
  simple SVGs as shapes, native tables, native bullet/numbered lists,
  hyperlinks (incl. outline links between slides), speaker notes from
  Touying, embedded fonts (TrueType and, since 0.1.1, CFF/OpenType
  converted to TrueType; licence permitting).
- What can't be expressed natively (equations, skewed/mirrored content,
  tiling fills, …) is exported as a picture at the exact position
  ("baked") and listed in the report with its source position.
- Checked in PowerPoint 365 on Windows: text within ~0.5 pt of Typst,
  tables without row growth.

Not yet (later phases, no tino change needed unless noted):

- **Animations (Phase 4, 0.2.0).** Up to 0.1.1, `--animations auto`
  behaves like `slides` (every Touying subslide is its own slide); from
  0.2.0 it exports click animations. tino keeps passing `auto`.
- **Slide masters/layouts (Phase 5).** Backgrounds, headers and footers are
  repeated on every slide for now.

## 2. Release assets

Release `vX.Y.Z` on https://github.com/Steve19802/typst-pptx/releases:

```
typst-pptx-x86_64-unknown-linux-musl.tar.xz      (static)
typst-pptx-aarch64-unknown-linux-musl.tar.xz     (static)
typst-pptx-armv7-unknown-linux-musleabi.tar.xz   (static, soft-float)
typst-pptx-riscv64gc-unknown-linux-gnu.tar.xz    (glibc)
SHA256SUMS
```

URL: `https://github.com/Steve19802/typst-pptx/releases/download/v${VERSION}/typst-pptx-${TARGET}.tar.xz`.
The target names are exactly typst's, so tino's existing `TYPST_TARGET`
switch can be reused. The archive layout mirrors typst's:

```
typst-pptx-x86_64-unknown-linux-musl/
  typst-pptx
  LICENSE  NOTICE  README.md
```

Dockerfile sketch (same `RUN` as the typst download, after `TYPST_TARGET`
is set):

```dockerfile
# renovate: datasource=github-releases depName=Steve19802/typst-pptx
ARG TYPST_PPTX_VERSION=0.2.0
...
    && curl -fsSL "https://github.com/Steve19802/typst-pptx/releases/download/v${TYPST_PPTX_VERSION}/typst-pptx-${TYPST_TARGET}.tar.xz" \
       | tar -xJ -C /tmp \
    && install -m 755 "/tmp/typst-pptx-${TYPST_TARGET}/typst-pptx" /usr/local/bin/typst-pptx \
    && rm -rf "/tmp/typst-pptx-${TYPST_TARGET}" \
```

Size: ~15 MB per archive, ~44 MB installed.

## 3. Version pairing

```
$ typst-pptx --version
typst-pptx 0.1.0 (typst 0.15.1)
```

Regex: `^typst-pptx (\S+) \(typst (\S+)\)$`. The Typst version is the exact
crate version the binary was built with. Rule: the image's
`typst --version` (`typst 0.15.1 (…)`) must report the same Typst version;
tino's typst-update workflow bumps `TYPST_PPTX_VERSION` to the newest
release built for the new Typst (release titles read `vX.Y.Z (typst A.B.C)`),
and an image check fails if the two differ. This repo's own typst-update
workflow opens the matching PR when Typst releases; a release follows.

## 4. Command line

```
typst-pptx compile [OPTIONS] <INPUT> [OUTPUT]
```

`OUTPUT` defaults to the input with `.pptx`. World options and their
environment variables are the same as typst's:

| Option | Env | Notes |
|---|---|---|
| `--root DIR` | `TYPST_ROOT` | default: the input's directory |
| `--font-path P1:P2` | `TYPST_FONT_PATHS` | `:`-separated |
| `--ignore-system-fonts` | `TYPST_IGNORE_SYSTEM_FONTS` | |
| `--ignore-embedded-fonts` | `TYPST_IGNORE_EMBEDDED_FONTS` | |
| `--package-path DIR` | `TYPST_PACKAGE_PATH` | |
| `--package-cache-path DIR` | `TYPST_PACKAGE_CACHE_PATH` | packages are downloaded like typst does (HTTPS, rustls) |
| `--input KEY=VALUE` | | repeatable, `sys.inputs` |
| `--creation-timestamp TS` | `SOURCE_DATE_EPOCH` | fixes document dates; output is deterministic |
| `--diagnostic-format human\|short` | | default `human`; tino should pass `short` |

Export options (defaults are what tino should use):

| Option | Values | Default |
|---|---|---|
| `--layouts` | `auto`, `none` | `auto` (0.3.0); `none` = every slide keeps its repeated content |
| `--animations` | `auto`, `none`, `slides` | `auto` (see §1); `none` = last subslide of each logical slide only |
| `--text` | `exact`, `flow` | `exact` (Typst's line breaks); `flow` lets PowerPoint wrap |
| `--embed-fonts` | `auto`, `never` | `auto` |
| `--tables` | `auto`, `native`, `shapes` | `auto` |
| `--svg` | `auto`, `picture`, `native` | `auto` (`--svg-max-nodes 60`) |
| `--report FILE.json` | | always pass it; see §6 |
| `--text-model` | `powerpoint`, `libreoffice` | `powerpoint`; don't change |
| `--emit-ir FILE.json` | | debugging only |

Suggested tino invocation (mirrors `_build_cmd()` for typst):

```
typst-pptx compile --diagnostic-format short \
  --root <project dir> --font-path <font dirs> \
  --package-path <pkg dir> --package-cache-path <cache dir> \
  --animations <auto|none|slides> --report <tmp>/report.json \
  <entry .typ> <tmp>/out.pptx
```

Timing: the 17-slide signal-wave demo takes 2.7 s and produces 1.0 MB
(embedded fonts included). Decks with many baked items render PNGs; use a
longer timeout than for PDF (e.g. 3×).

### 4.1 Fonts (tino must provide them)

typst-pptx finds fonts exactly like `typst compile`: `--font-path`
directories, system fonts (unless `--ignore-system-fonts`) and Typst's
built-in fonts. A deck whose fonts aren't found falls back to Typst's
built-in **Libertinus Serif**, in the PDF and in the `.pptx` alike, with
Typst's warning per missing family:

```
fixtures/themes/signal-wave/demo.typ:5:16: warning: unknown font family: ibm plex sans
```

Seen on 2026-10-22 on an Ubuntu tino install: the signal-wave demo came out
in Libertinus and, with 0.1.0, PowerPoint reported "Libertinus Serif" and
"Libertinus Serif Semibold" as missing (0.1.0 could not embed CFF fonts;
0.1.1 does). So:

1. The theme's fonts must reach typst-pptx: install them in the image
   (e.g. `/usr/share/fonts`) or ship them with the project and pass their
   directory with `--font-path` (same paths as for `typst compile`).
2. Surface `unknown font family` warnings to the user (UI toast or MCP
   result), not only in logs: the export succeeds, but not in the theme's
   look.
3. `report.json` `fonts.embedded` lists the faces actually used; a family
   the theme expects that is missing there is the same signal.

PowerPoint for the web edits with Microsoft's cloud fonts only: embedded
fonts show in its slide thumbnails and in the desktop app, but its
editing canvas substitutes any font that isn't a cloud font (IBM Plex is
one; Libertinus and New Computer Modern aren't). Nothing in the `.pptx` can
change that; users who edit in the browser need themes on cloud fonts.

Embedded fonts add to the file size: ~0.2 MB per IBM Plex face, ~0.6 MB per
Libertinus Serif face, ~1 MB per New Computer Modern face (no subsetting
yet).

## 5. Exit codes and stderr

| Exit | Meaning | Output file |
|---|---|---|
| 0 | success, possibly with warnings | written |
| 1 | compile error (Typst diagnostics) or input not found | not written |
| 2 | export error (e.g. unsupported Touying version) | not written |

Diagnostics are on stderr in typst's format. With `--diagnostic-format
short`, one line each, paths relative to the root, packages as
`@preview/name:version/path`, columns counted like typst's (characters
before the position):

```
fixtures/decks/fallbacks.typ:9:32: warning: exported as a picture: scaled, skewed or mirrored text
fixtures/decks/fallbacks.typ:13:90: warning: exported as a picture: text with gradient or tiling fill
out/scratch/rel/broken.typ:1:9: error: unclosed delimiter
```

Some messages have **no location** (same in typst for some errors); the
parser must accept them:

```
error: typst-pptx needs Touying 0.8 or newer (found 0.7.4)
  = hint: update the import to @preview/touying:0.8.0; breaking changes are listed at https://touying-typ.github.io/docs/changelog
error: input file not found (No such file or directory (os error 2)): nope.typ
warning: font "<family>" is not embedded: its licence forbids embedding (fsType restricted)
```

Hint lines start with two spaces and `= hint:` (also in short format for
location-less messages). Human format (for logs) looks like typst's:

```
warning: table exported as shapes and text boxes
   ┌─ fixtures/decks/tables.typ:39:2
   │
39 │   table(columns: 2, [Image], image("assets/gradient.png", width: 60pt)),
   │   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   │
   = hint: not a PowerPoint table: a cell holds a shape or an image
```

Export warning messages (all at the element's source position), for
display or filtering:

- `exported as a picture: <reason>` (baked; equations say `equation`)
- `paragraph exported line by line`
- `table exported as shapes and text boxes`
- `list exported without native bullets`
- `font "<family>" is not embedded: <reason>` (no location)

Typst's own warnings (unknown font family, …) appear as with `typst
compile`.

## 6. report.json

Written when `--report` is given and the export succeeds. Fields are only
added in later versions, never renamed or removed without a major version.

```json
{
  "typst_pptx": "0.1.0",
  "animations": "auto",
  "text_model": "powerpoint",
  "slides": [
    {"slide": 1, "page": 1, "pages": [1, 2, 3], "clicks": 2,
     "transition": null, "logical": "1",
     "items": {"native": 2, "baked": 7}, "notes": false}
  ],
  "baked": [
    {"slide": 1, "reason": "scaled, skewed or mirrored text",
     "file": "fixtures/decks/fallbacks.typ", "line": 9, "column": 32}
  ],
  "line_mode": [],
  "tables_as_shapes": [
    {"slide": 14, "reason": "a cell holds an equation",
     "file": "fixtures/themes/signal-wave/demo.typ", "line": 301, "column": 6}
  ],
  "lists_as_text": [],
  "fonts": {
    "embedded": ["IBM Plex Sans Bold", "IBM Plex Sans"],
    "not_embedded": [{"font": "Some Font", "reason": "its licence forbids embedding (fsType restricted)"}]
  }
}
```

- `slides`: one per slide in the `.pptx`; `page` is the 1-based Typst page
  (the first one, with animations), `pages` (0.2.0) every Typst page the
  slide shows (one per click state), `clicks` (0.2.0) the number of clicks,
  `transition` (0.2.0) `"morph"` or `null`, `logical` Touying's slide
  number; `items` counts items by fidelity
  (`native`, `normalized`, `approximate`, `baked`; absent keys = 0).
- `baked`, `line_mode`, `tables_as_shapes`, `lists_as_text`: entries with
  the 1-based slide and the source position (`file`, 1-based `line`,
  `column` as in the diagnostics). Positions inside packages use the
  `@preview/…` file name.
- For the `X-Tino-Pptx-Fallbacks` header: `len(report["baked"])`. The
  signal-wave demo bakes its 29 equations (29 is expected, not a problem).

## 7. tino checklist (plan §10, as built)

- Dockerfile: §2; image check `typst --version` vs `typst-pptx --version`
  (§3); typst-update workflow bumps both.
- `tino/config.py`: `TINO_PPTX_BINARY` (default `typst-pptx`, resolved with
  `shutil.which`; missing → feature off, not a startup error).
- `tino/services/compiler.py`: `pptx_version()` (cached, parse §3),
  `compile_pptx(slug, path, animations)` like `compile_pdf`; command per §4
  sharing font/package path logic with `_build_cmd()`; exit codes §5; same
  `_strip_paths` + `parse_diagnostics` (must accept location-less lines and
  `= hint:` lines); read and return `report.json`; longer timeout.
- `tino/routers/compile.py`: `GET …/compile/pptx/{path}?animations=auto|none|slides`
  → `FileResponse` with
  `application/vnd.openxmlformats-officedocument.presentationml.presentation`,
  `X-Tino-Pptx-Fallbacks`; 501 when unavailable; compile errors (exit 1)
  and export errors (exit 2) answered like PDF compile errors.
- `tino/routers/misc.py`: `pptx_version` in `/health` and `/api/config`
  (null when unavailable).
- `tino/mcp/server.py`: `compile_pptx(bucket, path, animations="auto")` →
  `{success, typst_pptx_version, pptx: <base64>, size, report}`, viewer
  role, error shape as `compile_pdf`; one line in `instructions.py` (read
  `report.baked` to see what isn't editable).
- UI: "Generate PowerPoint" in the preview's split button when
  `/api/config` has `pptx_version`; animation mode as a sub-option (default
  auto); toast with the fallback count from the header.
- Docs: editor, API, MCP, deployment pages; ADR
  `docs/development/adr/11-pptx-export.rst` (separate Rust exporter on
  Typst's frames, no PDF/SVG intermediate, version coupling).
- Tests: command building, diagnostics parsing with the stderr samples
  above, router 200/404/422/501, MCP success/failure, config exposure,
  binary faked; one integration test with the real binary when present.

## 8. Test material

- Real decks: `fixtures/themes/signal-wave/demo.typ` (Touying 0.8 deck,
  17 slides, tables, lists, equations, notes) with `fixtures/fonts`
  (IBM Plex) and `fixtures/packages` (vendored Touying, offline):
  `typst-pptx compile --root . --font-path fixtures/fonts --ignore-system-fonts --package-path fixtures/packages --package-cache-path /nonexistent fixtures/themes/signal-wave/demo.typ`
- Error cases: `fixtures/decks/errors/touying-0.7.typ` (exit 2),
  any syntax error (exit 1), `fixtures/decks/fallbacks.typ` (exit 0 with
  baked warnings).
- A fake binary for unit tests only needs to: print the `--version` line,
  write the output file and `--report` JSON, emit stderr lines as in §5 and
  exit with the right code.

## 9. Contract changes

Requests (new flags, report fields, message changes) go to this repository;
it updates this document and the CLI together and releases. Additive
changes only within 0.x minor releases; anything that breaks tino's parsing
gets a new minor version and a note at the top of this file.
