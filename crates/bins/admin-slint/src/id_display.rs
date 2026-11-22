//! Display-only digit normalization for identifiers shown by the Admin Slint host
//! (Q-16 / U-05). Delegates to the shared `client-composition` helper, which the
//! Desktop Slint host already depends on, so both hosts use one implementation.
//!
//! The conversion is for read-only view models only. Canonical IDs, API payloads
//! and editable inputs keep their original characters.

pub(crate) fn display_id(value: &str) -> String {
    client_composition::id_display::latin_digits(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persian_digits_in_a_display_id_become_latin() {
        assert_eq!(display_id("hold-۰۱۲"), "hold-012");
    }

    #[test]
    fn arabic_indic_digits_in_a_display_id_become_latin() {
        assert_eq!(display_id("hold-١٢٣"), "hold-123");
    }

    #[test]
    fn canonical_latin_ids_are_unchanged() {
        assert_eq!(display_id("st26sm0035"), "st26sm0035");
        assert_eq!(
            display_id("3f2504e0-4f89-11d3-9a0c-0305e82c3301"),
            "3f2504e0-4f89-11d3-9a0c-0305e82c3301"
        );
    }

    #[test]
    fn non_digit_characters_are_preserved() {
        assert_eq!(display_id("سهیل-۲۶"), "سهیل-26");
    }
}
