//! ONYX Desktop Slint host.
//!
//! Authentication uses the shared `desktop-session` authority: three-field
//! organization login, org registry lookup, per-server sessions, and a forced
//! password-change screen. Tokens and passwords never become Slint properties.

use std::sync::Arc;

use composition::{LoginFailure, PasswordFailure};

pub mod composition;
pub mod device;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let composition = runtime
        .block_on(composition::SlintComposition::open())
        .map_err(|e| std::io::Error::other(format!("failed to initialize ONYX: {e}")))?;
    let ui = DesktopWindow::new()?;
    if let Some(label) = option_env!("ONYX_TEST_BUILD_LABEL") {
        ui.set_build_label(format!(" — {label}").into());
    }

    // A restored session can belong to a different server; surface it instead
    // of showing the login pane while a valid session is live.
    if let Some(summary) = runtime.block_on(composition.current_summary()) {
        ui.set_authenticated(true);
        ui.set_display_name(summary.username.into());
        ui.set_must_change_password(summary.must_change_password);
        ui.set_status("Restored a secure session.".into());
    }

    let login_composition = Arc::clone(&composition);
    let login_handle = runtime.handle().clone();
    let login_weak = ui.as_weak();
    ui.on_login_requested(move |organization, username, password| {
        if let Some(ui) = login_weak.upgrade() {
            ui.set_busy(true);
            ui.set_status("Looking up the organization and authenticating…".into());
        }

        let composition = Arc::clone(&login_composition);
        let weak = login_weak.clone();
        let organization = organization.to_string();
        let username = username.to_string();
        let password = password.to_string();
        login_handle.spawn(async move {
            let result = composition.login(&organization, &username, &password).await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_busy(false);
                    ui.set_password("".into());
                    match result {
                        Ok(summary) => {
                            ui.set_authenticated(true);
                            ui.set_display_name(summary.username.into());
                            ui.set_must_change_password(summary.must_change_password);
                            if summary.must_change_password {
                                ui.set_status("Your account requires a new password.".into());
                            } else {
                                ui.set_status(
                                    "Authenticated. The full application runtime remains gated."
                                        .into(),
                                );
                            }
                        }
                        Err(error) => {
                            let message = match error {
                                LoginFailure::OrganizationNotFound => "Organization not found.",
                                LoginFailure::InvalidOrganizationServer => {
                                    "The organization's server address is invalid."
                                }
                                LoginFailure::InvalidCredentials => "Invalid username or password.",
                                LoginFailure::Network => "Could not reach the server.",
                                LoginFailure::Server => {
                                    "Sign-in failed. Verify credentials and network connectivity."
                                }
                            };
                            ui.set_status(message.into());
                        }
                    }
                }
            });
        });
    });

    let change_composition = Arc::clone(&composition);
    let change_handle = runtime.handle().clone();
    let change_weak = ui.as_weak();
    ui.on_change_password_requested(move |current_password, new_password| {
        if let Some(ui) = change_weak.upgrade() {
            ui.set_busy(true);
            ui.set_status("Changing password…".into());
        }

        let composition = Arc::clone(&change_composition);
        let weak = change_weak.clone();
        let current_password = current_password.to_string();
        let new_password = new_password.to_string();
        change_handle.spawn(async move {
            let result = composition
                .change_password(&current_password, &new_password)
                .await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_busy(false);
                    ui.set_current_password("".into());
                    ui.set_new_password("".into());
                    match result {
                        Ok(()) => {
                            // The Worker revoked every session on success.
                            ui.set_authenticated(false);
                            ui.set_must_change_password(false);
                            ui.set_display_name("".into());
                            ui.set_status("Password changed. Sign in again.".into());
                        }
                        Err(error) => {
                            let message = match error {
                                PasswordFailure::CurrentPasswordIncorrect => {
                                    "The current password is incorrect."
                                }
                                PasswordFailure::WeakPassword => {
                                    "The new password must contain at least 12 characters."
                                }
                                PasswordFailure::PasswordUnchanged => {
                                    "The new password must be different from the current one."
                                }
                                PasswordFailure::SessionExpired => {
                                    "Sign in again to change your password."
                                }
                                PasswordFailure::Network => "Could not reach the server.",
                                PasswordFailure::Server => {
                                    "The password could not be changed. Try again."
                                }
                            };
                            ui.set_status(message.into());
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
                    ui.set_must_change_password(false);
                    ui.set_display_name("".into());
                    ui.set_organization("".into());
                    ui.set_username("".into());
                    ui.set_password("".into());
                    ui.set_current_password("".into());
                    ui.set_new_password("".into());
                    ui.set_status("Signed out.".into());
                }
            });
        });
    });

    ui.run()?;
    Ok(())
}
