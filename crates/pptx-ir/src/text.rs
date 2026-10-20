//! Where PowerPoint puts baselines in a text box with zero insets and no
//! autofit (plan §5.2).
//!
//! Measured with LibreOffice 25.2 (`tools/validate/calibrate_baseline.py`):
//! the model is independent of the font's metrics. A line is 1.2 em high and
//! its baseline sits 0.2 em above its bottom; with exact spacing `S` the line
//! is `S` high and the baseline still 0.2 em above its bottom. The PowerPoint
//! side is confirmed at the Windows milestone check.

/// Single line height as a multiple of the font size.
pub const SINGLE_LINE: f64 = 1.2;
/// Distance from the baseline to the line bottom, as a multiple of the size.
pub const DESCENT: f64 = 0.2;

/// Distance from the box top to the first baseline, in pt, for a first line
/// whose largest font size is `size` pt and an exact line spacing `spacing`
/// pt (`None` = single).
pub fn first_baseline(size: f64, spacing: Option<f64>) -> f64 {
    spacing.unwrap_or(size * SINGLE_LINE) - size * DESCENT
}

/// Height of a box holding `lines` lines, in pt.
pub fn box_height(size: f64, spacing: Option<f64>, lines: usize) -> f64 {
    spacing.unwrap_or(size * SINGLE_LINE) * lines as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_spacing_puts_the_baseline_one_em_down() {
        assert!((first_baseline(24.0, None) - 24.0).abs() < 1e-9);
    }

    #[test]
    fn exact_spacing_keeps_the_descent() {
        assert!((first_baseline(10.0, Some(16.0)) - 14.0).abs() < 1e-9);
    }
}
