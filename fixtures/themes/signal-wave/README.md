# signal-wave (Touying 0.8 port)

Reference theme for the exporter (plan §3). Copied from
[seaslides](https://github.com/touying-typ/seaslides) at commit
`dcde5bf6c6875b300e2999fc7766f369a13a67ac`,
`skills/seaslides-typst-slides-skill/templates/themes/custom-rich/signal-wave/`
(MIT, see `LICENSE`).

Changes from upstream, nothing else:

| File | Change | Why |
|---|---|---|
| both | `@preview/touying:0.7.4` → `0.8.0` | exporter supports Touying 0.8 only |
| `template.typ` | `font: "Menlo"` → `font: ("Menlo", "IBM Plex Mono")` | Menlo is macOS-only; without a fallback the code blocks pick an arbitrary font |
| `demo.typ` | `#figure[` → `#figure(caption: none)[` | Touying 0.8.0 fails on caption-less figures under Typst 0.15 (touying#434/#445, fixed on main, unreleased) |

Fonts come from `fixtures/fonts/ibm-plex/` only. The `Arial`, `PingFang SC`
and `Menlo` warnings are expected.

Upstream IBM Plex (v3.004/3.005 TTFs) names its Medium and SemiBold faces
"IBM Plex Sans Medm" / "IBM Plex Sans SmBld" in name ID 1. Typst groups faces by
name ID 1 (minus known style suffixes), so `weight: "medium"`/`"semibold"` on
"IBM Plex Sans" resolve to Regular/Bold, both in the PDF and in the export.
