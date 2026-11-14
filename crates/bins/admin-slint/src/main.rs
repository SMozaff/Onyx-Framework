//! ONYX Admin native UI (Slint). Phase 1: sign-in with memory-only tokens.

mod auth;
mod i18n;
mod i18n_catalog;

use auth::{AdminSession, LoginError};
use i18n::{text, Lang};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

slint::include_modules!();

fn apply_language(ui: &AppWindow, lang: Lang) {
    let s = ui.global::<Strings>();
    s.set_app_title(text(lang, "auth.adminTitleBar").into());
    s.set_theme_light(text(lang, "theme.light").into());
    s.set_theme_dark(text(lang, "theme.dark").into());
    s.set_english(text(lang, "common.english").into());
    s.set_farsi(text(lang, "common.farsi").into());
    s.set_admin_title(text(lang, "auth.adminTitle").into());
    s.set_sign_in_heading(text(lang, "auth.adminSignIn").into());
    s.set_sign_in_description(text(lang, "auth.useAssignedCredentials").into());
    s.set_username_label(text(lang, "auth.username").into());
    s.set_password_label(text(lang, "auth.password").into());
    s.set_sign_in_button(text(lang, "auth.signIn").into());
    s.set_signing_in_button(text(lang, "auth.signingIn").into());
    s.set_signed_in_label(text(lang, "auth.signedIn").into());
    s.set_sign_out_button(text(lang, "auth.signOut").into());
}

/// Maps a failure to catalog text. Network and server failures use existing
/// catalog wording; a dedicated "sign-in failed, try again" key is not in the
/// catalog yet.
fn error_text(lang: Lang, error: LoginError) -> &'static str {
    match error {
        LoginError::InvalidCredentials => text(lang, "auth.adminInvalidCredentials"),
        LoginError::Timeout | LoginError::Network => text(lang, "common.connectionLost"),
        LoginError::Server { .. } | LoginError::InvalidResponse => text(lang, "status.failed"),
    }
}

fn new_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("admin-{nanos}")
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    apply_language(&ui, Lang::En);
    let lang = Rc::new(Cell::new(Lang::En));
    // Memory-only session slot (Q-01): never written to disk or the keyring.
    let session: Arc<Mutex<Option<AdminSession>>> = Arc::new(Mutex::new(None));

    let weak = ui.as_weak();
    let lang_for_switch = lang.clone();
    ui.on_select_language(move |tag| {
        // An unsupported tag is ignored and the current language stays active.
        if let (Some(ui), Some(new_lang)) = (weak.upgrade(), Lang::from_tag(&tag)) {
            lang_for_switch.set(new_lang);
            apply_language(&ui, new_lang);
        }
    });

    let sign_out_session = session.clone();
    let weak = ui.as_weak();
    ui.on_sign_in(move |username, password| {
        let Some(ui) = weak.upgrade() else { return };
        if ui.get_busy() {
            return;
        }
        ui.set_busy(true);
        ui.set_error_text("".into());

        let current_lang = lang.get();
        let username = username.to_string();
        let password = password.to_string();
        let request_id = new_request_id();
        let weak = weak.clone();
        let session = session.clone();

        std::thread::spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| LoginError::Network)
                .and_then(|rt| rt.block_on(auth::sign_in(&username, &password, &request_id)));

            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = weak.upgrade() else { return };
                ui.set_busy(false);
                match result {
                    Ok(new_session) => {
                        ui.set_signed_in_as(new_session.username.clone().into());
                        ui.set_signed_in(true);
                        if let Ok(mut slot) = session.lock() {
                            *slot = Some(new_session);
                        }
                    }
                    Err(error) => ui.set_error_text(error_text(current_lang, error).into()),
                }
            });
        });
    });

    let weak = ui.as_weak();
    let session = sign_out_session;
    ui.on_sign_out(move || {
        // Dropping the session clears the in-memory tokens.
        if let Ok(mut slot) = session.lock() {
            *slot = None;
        }
        if let Some(ui) = weak.upgrade() {
            ui.set_signed_in(false);
            ui.set_signed_in_as("".into());
            ui.set_error_text("".into());
        }
    });

    ui.run()
}
