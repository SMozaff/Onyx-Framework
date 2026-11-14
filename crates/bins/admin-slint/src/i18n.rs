//! Admin UI strings. Text comes from the generated canonical catalog
//! (`i18n_catalog.rs`, produced from `shared/i18n/{en,fa}.json`), so the Slint
//! hosts and the web frontends share one source of truth.

use crate::i18n_catalog::{EN, FA};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Fa,
}

impl Lang {
    pub fn from_tag(tag: &str) -> Option<Lang> {
        match tag {
            "en" => Some(Lang::En),
            "fa" => Some(Lang::Fa),
            _ => None,
        }
    }
}

/// Catalog text for `key` in `lang`. Falls back to English, then to the key
/// itself, so a missing key is visible instead of silently blank.
pub fn text(lang: Lang, key: &str) -> &'static str {
    let table = match lang {
        Lang::En => EN,
        Lang::Fa => FA,
    };
    lookup(table, key)
        .or_else(|| lookup(EN, key))
        .unwrap_or_else(|| {
            debug_assert!(false, "missing i18n key: {key}");
            ""
        })
}

fn lookup(table: &'static [(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_tags_parse() {
        assert_eq!(Lang::from_tag("en"), Some(Lang::En));
        assert_eq!(Lang::from_tag("fa"), Some(Lang::Fa));
        assert_eq!(Lang::from_tag("xx"), None);
    }

    #[test]
    fn sign_in_strings_resolve_in_both_languages() {
        assert_eq!(text(Lang::En, "auth.signIn"), "Sign in");
        assert_ne!(text(Lang::Fa, "auth.signIn"), "");
        assert_eq!(text(Lang::En, "auth.adminTitleBar"), "ONYX Admin");
    }

    #[test]
    fn both_tables_have_the_same_keys() {
        assert_eq!(EN.len(), FA.len());
        for (key, _) in EN {
            assert!(lookup(FA, key).is_some(), "{key} missing in fa");
        }
    }
}
