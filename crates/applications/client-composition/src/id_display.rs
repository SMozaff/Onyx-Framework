//! Display-only digit normalization for identifiers (Q-16 / U-05).
//!
//! Decision (2026-10-27): IDs are shown with Latin digits in every locale.
//! This is display-only. Stored values, serialization, identifiers, and API
//! data are never changed. Call this only at the rendering boundary.

/// Returns `s` with Persian (U+06F0–U+06F9) and Arabic-Indic (U+0660–U+0669)
/// digits replaced by ASCII digits. All other characters are unchanged.
pub fn latin_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{06F0}'..='\u{06F9}' => char::from(b'0' + (c as u32 - 0x06F0) as u8),
            '\u{0660}'..='\u{0669}' => char::from(b'0' + (c as u32 - 0x0660) as u8),
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persian_digits_become_latin() {
        assert_eq!(latin_digits("۰۱۲۳۴۵۶۷۸۹"), "0123456789");
    }

    #[test]
    fn arabic_indic_digits_become_latin() {
        assert_eq!(latin_digits("٠١٢٣٤٥٦٧٨٩"), "0123456789");
    }

    #[test]
    fn id_with_latin_letters_and_digits_is_unchanged() {
        assert_eq!(latin_digits("st26sm0035"), "st26sm0035");
        assert_eq!(latin_digits("tm261030sm0007"), "tm261030sm0007");
    }

    #[test]
    fn persian_letters_are_not_altered() {
        assert_eq!(latin_digits("سهیل ۲۶"), "سهیل 26");
    }

    #[test]
    fn is_idempotent() {
        let once = latin_digits("۰۰۳۵");
        assert_eq!(latin_digits(&once), once);
    }
}
