// Role hints for typst-pptx (plan §7). Without typst-pptx they do nothing:
// the markers are invisible metadata and leave the layout unchanged.

/// The roles typst-pptx turns into PowerPoint placeholders.
#let roles = ("title", "subtitle", "body", "footer", "date", "slide-number")

/// Marks `body` as the slide's `kind` (one of `roles`). typst-pptx exports
/// its text as that placeholder of the slide layout; a `"slide-number"`
/// becomes PowerPoint's slide number field where the numbers agree.
///
/// ```typ
/// #import "@local/pptx-hints:0.1.0" as pptx
/// #pptx.role("title", heading-text)
/// ```
#let role(kind, body) = {
  assert(
    kind in roles,
    message: "pptx-hints: unknown role " + repr(kind) + ", expected one of " + roles.join(", "),
  )
  [#metadata((kind: kind)) <pptx-role-start>#body#metadata(kind) <pptx-role-end>]
}
