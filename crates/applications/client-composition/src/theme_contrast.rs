//! WCAG 2.x contrast checks for the shared UI colour tokens (U-12).
//!
//! Token values are copied from the Slint sources:
//! - `crates/bins/desktop-slint/ui/main.slint` (feasibility palette)
//! - `crates/bins/admin-slint/ui/app.slint` (theme tokens, light and dark)
//! Text needs at least 4.5:1 (WCAG AA, normal text).

/// Relative luminance per WCAG 2.x.
pub fn relative_luminance(hex: &str) -> f64 {
    let h = hex.trim_start_matches('#');
    let channel = |i: usize| -> f64 {
        let v = u8::from_str_radix(&h[i..i + 2], 16).expect("hex") as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4)
}

/// WCAG contrast ratio between two colours, always >= 1.
pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

pub const AA_NORMAL_TEXT: f64 = 4.5;

/// (name, foreground, background)
pub const TEXT_PAIRS: &[(&str, &str, &str)] = &[
    ("dark text on background", "#e6edf5", "#0f1720"),
    ("dark text on surface", "#e6edf5", "#17212d"),
    ("dark text-dim on background", "#9fb0c3", "#0f1720"),
    ("dark text-dim on surface", "#9fb0c3", "#17212d"),
    ("light text on background", "#102b4d", "#f3f7fb"),
    ("light text on surface", "#102b4d", "#ffffff"),
    ("light text-dim on background", "#5d7088", "#f3f7fb"),
    ("light text-dim on surface", "#5d7088", "#ffffff"),
    ("desktop text on background", "#e6edf7", "#10141f"),
    ("desktop muted text on background", "#9eacc4", "#10141f"),
    ("accent with white label", "#ffffff", "#1f6098"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_on_white_is_21_to_1() {
        let r = contrast_ratio("#000000", "#ffffff");
        assert!((r - 21.0).abs() < 0.01, "{r}");
    }

    #[test]
    fn every_text_pair_meets_wcag_aa_normal_text() {
        let failures: Vec<String> = TEXT_PAIRS
            .iter()
            .filter_map(|(name, fg, bg)| {
                let r = contrast_ratio(fg, bg);
                (r < AA_NORMAL_TEXT).then(|| format!("{name}: {r:.2}:1"))
            })
            .collect();
        assert!(failures.is_empty(), "below 4.5:1: {failures:?}");
    }
}
