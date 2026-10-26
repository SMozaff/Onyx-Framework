//! Translation table for the Admin UI. Every key must have a value for every
//! language: the `match` below is exhaustive, so a missing pair fails to compile.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    AppTitle,
    LightMode,
    DarkMode,
    Phase0Notice,
}

#[cfg(test)]
pub const ALL_KEYS: &[Key] = &[
    Key::AppTitle,
    Key::LightMode,
    Key::DarkMode,
    Key::Phase0Notice,
];

pub fn text(lang: Lang, key: Key) -> &'static str {
    match (lang, key) {
        (Lang::En, Key::AppTitle) => "ONYX Admin",
        (Lang::Fa, Key::AppTitle) => "مدیریت ONYX",
        (Lang::En, Key::LightMode) => "Light mode",
        (Lang::Fa, Key::LightMode) => "حالت روشن",
        (Lang::En, Key::DarkMode) => "Dark mode",
        (Lang::Fa, Key::DarkMode) => "حالت تاریک",
        (Lang::En, Key::Phase0Notice) => "Phase 0 placeholder. Sign-in arrives in Phase 1.",
        (Lang::Fa, Key::Phase0Notice) => "جایگاه موقت فاز ۰. ورود در فاز ۱ اضافه می\u{200c}شود.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_non_empty_text_in_every_language() {
        for lang in [Lang::En, Lang::Fa] {
            for key in ALL_KEYS {
                assert!(
                    !text(lang, *key).is_empty(),
                    "empty text for {key:?} in {lang:?}"
                );
            }
        }
    }

    #[test]
    fn language_tags_parse() {
        assert_eq!(Lang::from_tag("en"), Some(Lang::En));
        assert_eq!(Lang::from_tag("fa"), Some(Lang::Fa));
        assert_eq!(Lang::from_tag("xx"), None);
    }
}
