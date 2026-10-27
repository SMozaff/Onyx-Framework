//! ONYX Desktop Slint host feasibility spike.
//!
//! The host now uses the real API session service and shared relay adapter.
//! It intentionally does not claim production parity: persistent OS-keyring
//! restore and the full client-composition/peer-sync lifecycle remain gated.

use std::sync::Arc;

pub mod authority;
pub mod composition;
pub mod session_service;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let composition = composition::SlintComposition::new()
        .map_err(|_| std::io::Error::other("failed to initialize ONYX session service"))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let ui = DesktopWindow::new()?;

    let login_composition = Arc::clone(&composition);
    let login_handle = runtime.handle().clone();
    let login_weak = ui.as_weak();
    ui.on_login_requested(move |username, password| {
        if let Some(ui) = login_weak.upgrade() {
            ui.set_busy(true);
            ui.set_status("Authenticating with ONYX…".into());
        }

        let composition = Arc::clone(&login_composition);
        let weak = login_weak.clone();
        let username = username.to_string();
        let password = password.to_string();
        login_handle.spawn(async move {
            let result = composition.login(&username, &password).await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_busy(false);
                    ui.set_password("".into());
                    match result {
                        Ok(identity) => {
                            ui.set_authenticated(true);
                            ui.set_display_name(identity.username.into());
                            ui.set_status(
                                "Authenticated. The full application runtime remains gated.".into(),
                            );
                        }
                        Err(_) => {
                            ui.set_status(
                                "Sign-in failed. Verify credentials and network connectivity."
                                    .into(),
                            );
                        }
                    }
                }
            });
        });
    });

    let logout_composition = Arc::clone(&composition);
    let logout_handle = runtime.handle().clone();
    let logout_weak = ui.as_weak();
    ui.on_logout_requested(move || {
        if let Some(ui) = logout_weak.upgrade() {
            ui.set_busy(true);
            ui.set_status("Signing out…".into());
        }

        let composition = Arc::clone(&logout_composition);
        let weak = logout_weak.clone();
        logout_handle.spawn(async move {
            composition.logout().await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_busy(false);
                    ui.set_authenticated(false);
                    ui.set_display_name("".into());
                    ui.set_username("".into());
                    ui.set_password("".into());
                    ui.set_status("Signed out.".into());
                }
            });
        });
    });

    ui.run()?;
    Ok(())
}
