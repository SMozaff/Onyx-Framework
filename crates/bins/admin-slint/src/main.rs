//! ONYX Admin native UI (Slint). Phase 0: shell, language and theme switches.

mod i18n;

use i18n::{text, Key, Lang};

slint::include_modules!();

fn apply_language(ui: &AppWindow, lang: Lang) {
    let strings = ui.global::<Strings>();
    strings.set_app_title(text(lang, Key::AppTitle).into());
    strings.set_light_mode(text(lang, Key::LightMode).into());
    strings.set_dark_mode(text(lang, Key::DarkMode).into());
    strings.set_phase0_notice(text(lang, Key::Phase0Notice).into());
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    apply_language(&ui, Lang::En);

    let weak = ui.as_weak();
    ui.on_select_language(move |tag| {
        // An unsupported tag is ignored and the current language stays active.
        if let (Some(ui), Some(lang)) = (weak.upgrade(), Lang::from_tag(&tag)) {
            apply_language(&ui, lang);
        }
    });
    ui.run()
}
