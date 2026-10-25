// Role hints for typst-pptx (plan §7). Without typst-pptx they do nothing:
// the markers are invisible metadata and leave the layout unchanged.

/// The roles typst-pptx turns into PowerPoint placeholders.
#let roles = ("title", "subtitle", "body", "footer", "date", "slide-number")

/// Marks `body` as the slide's `kind` (one of `roles`). typst-pptx exports
/// its text as that placeholder of the slide layout; a `"slide-number"`
/// becomes PowerPoint's slide number field where the numbers agree. Mark only
/// the number: text around it ("/ 17") then goes to the slide layout as fixed
/// text, and PowerPoint and LibreOffice both number the slides themselves.
///
/// ```typ
/// #import "@local/pptx-hints:0.2.0" as pptx
/// #pptx.role("title", heading-text)
/// ```
#let role(kind, body) = {
  assert(
    kind in roles,
    message: "pptx-hints: unknown role " + repr(kind) + ", expected one of " + roles.join(", "),
  )
  [#metadata((kind: kind)) <pptx-role-start>#body#metadata(kind) <pptx-role-end>]
}

/// Declares the theme's colours for PowerPoint's colour scheme (Design →
/// Variants), instead of letting typst-pptx infer them from use. Place it
/// once anywhere in the deck (e.g. in the theme's show rule); unset slots
/// are still inferred. Colours equal to a scheme colour are written as
/// theme references, so recolouring in PowerPoint changes them.
///
/// - dark, dark2: text colours (PowerPoint's dk1/tx1, dk2/tx2).
/// - light, light2: background colours (lt1/bg1, lt2/bg2).
/// - accents: up to six colours, most important first (accent1–6).
/// - link, visited-link: hyperlink colours.
///
/// ```typ
/// #pptx.palette(dark: navy, light: white, accents: (blue, orange, green))
/// ```
#let palette(
  dark: none,
  light: none,
  dark2: none,
  light2: none,
  accents: (),
  link: none,
  visited-link: none,
) = {
  assert(accents.len() <= 6, message: "pptx-hints: at most six accents, got " + str(accents.len()))
  let hex(c) = if c == none { none } else { c.to-hex() }
  [#metadata((
      dark: hex(dark),
      light: hex(light),
      dark2: hex(dark2),
      light2: hex(light2),
      accents: accents.map(hex),
      link: hex(link),
      visited-link: hex(visited-link),
    )) <pptx-palette>]
}
