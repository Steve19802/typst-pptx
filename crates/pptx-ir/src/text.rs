//! Where the target application puts baselines in a text box with zero
//! insets and no autofit (plan §5.2). Both models are independent of the
//! font's metrics.
//!
//! - PowerPoint 365 (2609), fitted on the Phase 0 milestone decks exported
//!   to PDF on Windows (240 lines, residual ≤ 0.2 pt single, ≤ 0.7 pt exact):
//!   first baseline 0.949 em below the top with single spacing, 0.744 × S
//!   with exact spacing S.
//! - LibreOffice 25.2 (`tools/validate/calibrate_baseline.py`, exact to
//!   0.001 em): a line is 1.2 em (or S) high with its baseline 0.2 em above
//!   the line bottom.
//!
//! Line pitch is S in both, and 1.2 em for single spacing. In LibreOffice
//! each line's baseline sits 0.2 × *that line's* largest size above its
//! bottom, so lines with different sizes can't share an exact spacing; in
//! PowerPoint they can (checked on the milestone decks, residual ≤ 0.5 pt).

/// Single line height as a multiple of the font size.
pub const SINGLE_LINE: f64 = 1.2;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextModel {
    #[default]
    PowerPoint,
    LibreOffice,
}

impl TextModel {
    /// Distance from the box top to the first baseline, in pt, for a first
    /// line whose largest font size is `size` pt and an exact line spacing
    /// `spacing` pt (`None` = single).
    pub fn first_baseline(self, size: f64, spacing: Option<f64>) -> f64 {
        match (self, spacing) {
            (Self::PowerPoint, None) => 0.949 * size,
            (Self::PowerPoint, Some(spacing)) => 0.744 * spacing,
            (Self::LibreOffice, spacing) => spacing.unwrap_or(size * SINGLE_LINE) - 0.2 * size,
        }
    }
}

impl TextModel {
    /// Whether lines of different font sizes can be in one paragraph.
    pub fn mixed_line_sizes(self) -> bool {
        self == Self::PowerPoint
    }
}

/// Height of a box holding `lines` lines, in pt.
pub fn box_height(size: f64, spacing: Option<f64>, lines: usize) -> f64 {
    spacing.unwrap_or(size * SINGLE_LINE) * lines as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libreoffice_single_spacing_puts_the_baseline_one_em_down() {
        let first = TextModel::LibreOffice.first_baseline(24.0, None);
        assert!((first - 24.0).abs() < 1e-9);
    }

    #[test]
    fn libreoffice_exact_spacing_keeps_the_descent() {
        let first = TextModel::LibreOffice.first_baseline(10.0, Some(16.0));
        assert!((first - 14.0).abs() < 1e-9);
    }

    #[test]
    fn powerpoint_exact_spacing_scales_with_the_spacing() {
        let first = TextModel::PowerPoint.first_baseline(10.0, Some(20.0));
        assert!((first - 14.88).abs() < 1e-9);
    }
}
