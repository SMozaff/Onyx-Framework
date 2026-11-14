//! ONYX Admin native UI (Slint). Phase 1: sign-in with memory-only tokens.

mod auth;
mod i18n;
mod i18n_catalog;
mod orgs;
mod users;

use auth::{AdminSession, LoginError};
use slint::{ModelRc, VecModel};
use users::UsersError;
use slint::Model;
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
    s.set_nav_users(text(lang, "nav.users").into());
    s.set_nav_settings(text(lang, "nav.settings").into());
    s.set_nav_profiles(text(lang, "nav.profiles").into());
    s.set_nav_organizations(text(lang, "nav.organizations").into());
    s.set_no_data(text(lang, "common.noData").into());
    s.set_status_active(text(lang, "status.active").into());
    s.set_status_inactive(text(lang, "common.inactive").into());
    s.set_loading(text(lang, "common.loading").into());
    s.set_create_user_button(text(lang, "common.createUser").into());
    s.set_email_label(text(lang, "settings.notificationsEmail").into());
    s.set_class_label(text(lang, "common.class").into());
    s.set_parent_label(text(lang, "common.parent").into());
    s.set_save_button(text(lang, "common.save").into());
    s.set_activate_button(text(lang, "common.activate").into());
    s.set_deactivate_button(text(lang, "common.deactivate").into());
    s.set_name_label(text(lang, "common.name").into());
    s.set_create_org_button(text(lang, "common.createOrganization").into());
    s.set_create_admin_button(text(lang, "common.createOrganizationAdmin").into());
    s.set_organization_label(text(lang, "common.organizations").into());
    s.set_admins_heading(text(lang, "common.organizationAdmins").into());
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

/// Runs one Users write on a worker thread, then reloads the list on success.
fn run_user_write<F, Fut>(
    weak: slint::Weak<AppWindow>,
    session: Arc<Mutex<Option<AdminSession>>>,
    lang: Lang,
    action: F,
) where
    F: FnOnce(String) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), UsersError>>,
{
    let token = match session
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|s| s.access_token().to_owned()))
    {
        Some(token) => token,
        None => return,
    };
    if let Some(ui) = weak.upgrade() {
        ui.set_users_loading(true);
        ui.set_users_error("".into());
    }
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| UsersError::Failed)
            .and_then(|rt| rt.block_on(action(token)));
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_users_loading(false);
            match result {
                Ok(()) => load_users(weak.clone(), session, lang),
                Err(UsersError::Forbidden) => {
                    ui.set_users_error(text(lang, "auth.adminAccessRequired").into())
                }
                Err(UsersError::Failed) => ui.set_users_error(text(lang, "status.failed").into()),
            }
        });
    });
}

type SessionSlot = Arc<Mutex<Option<AdminSession>>>;

fn show_error(ui: &AppWindow, lang: Lang, error: UsersError) {
    match error {
        UsersError::Forbidden => ui.set_users_error(text(lang, "auth.adminAccessRequired").into()),
        UsersError::Failed => ui.set_users_error(text(lang, "status.failed").into()),
    }
}

/// Runs one Organizations call on a worker thread. `done` runs on the UI thread
/// with the result, so reads can fill the screen and writes can reload it.
fn run_org_call<T, F, Fut, D>(
    weak: slint::Weak<AppWindow>,
    session: SessionSlot,
    lang: Lang,
    action: F,
    done: D,
) where
    T: Send + 'static,
    F: FnOnce(String) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, UsersError>>,
    D: FnOnce(slint::Weak<AppWindow>, SessionSlot, Lang, Result<T, UsersError>) + Send + 'static,
{
    let token = match session
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|s| s.access_token().to_owned()))
    {
        Some(token) => token,
        None => return,
    };
    if let Some(ui) = weak.upgrade() {
        ui.set_users_loading(true);
        ui.set_users_error("".into());
    }
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| UsersError::Failed)
            .and_then(|rt| rt.block_on(action(token)));
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_users_loading(false);
            }
            done(weak, session, lang, result);
        });
    });
}

/// Loads the organization list (All-Father only).
fn load_orgs(weak: slint::Weak<AppWindow>, session: SessionSlot, lang: Lang) {
    run_org_call(
        weak,
        session,
        lang,
        |token| async move { orgs::list_orgs(&token).await },
        move |weak, _session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(rows) => {
                    let items: Vec<OrgItem> = rows
                        .into_iter()
                        .map(|row| OrgItem {
                            id: row.id.into(),
                            name: row.name.into(),
                            active: row.is_active,
                        })
                        .collect();
                    ui.set_orgs(ModelRc::new(VecModel::from(items)));
                }
                Err(error) => show_error(&ui, lang, error),
            }
        },
    );
}

/// Loads the Admins of one organization and marks it as selected.
fn load_org_admins(weak: slint::Weak<AppWindow>, session: SessionSlot, lang: Lang, org_id: String) {
    if let Some(ui) = weak.upgrade() {
        let name = ui
            .get_orgs()
            .iter()
            .find(|org| org.id.as_str() == org_id)
            .map(|org| org.name.to_string())
            .unwrap_or_default();
        ui.set_selected_org_id(org_id.clone().into());
        ui.set_selected_org_name(name.into());
    }
    let call_id = org_id.clone();
    run_org_call(
        weak,
        session,
        lang,
        move |token| async move { orgs::list_admins(&token, &call_id).await },
        move |weak, _session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(rows) => {
                    let items: Vec<AdminItem> = rows
                        .into_iter()
                        .map(|row| AdminItem {
                            id: row.id.into(),
                            username: row.username.into(),
                            active: row.is_active,
                        })
                        .collect();
                    ui.set_org_admins(ModelRc::new(VecModel::from(items)));
                }
                Err(error) => show_error(&ui, lang, error),
            }
        },
    );
}

/// Runs an Organizations write, then reloads the organization list, or the
/// selected organization's Admins when `admins_of` is set.
fn org_write<F, Fut>(
    weak: slint::Weak<AppWindow>,
    session: SessionSlot,
    lang: Lang,
    action: F,
    admins_of: Option<String>,
) where
    F: FnOnce(String) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), UsersError>>,
{
    run_org_call(weak, session, lang, action, move |weak, session, lang, result| {
        let Some(ui) = weak.upgrade() else { return };
        match result {
            Ok(()) => match admins_of {
                Some(org_id) => load_org_admins(weak.clone(), session, lang, org_id),
                None => load_orgs(weak.clone(), session, lang),
            },
            Err(error) => show_error(&ui, lang, error),
        }
    });
}

/// Loads the Users list with the in-memory token. Runs off the UI thread.
fn load_users(weak: slint::Weak<AppWindow>, session: Arc<Mutex<Option<AdminSession>>>, lang: Lang) {
    let token = match session
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|s| s.access_token().to_owned()))
    {
        Some(token) => token,
        None => return,
    };
    if let Some(ui) = weak.upgrade() {
        ui.set_users_loading(true);
        ui.set_users_error("".into());
    }
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| UsersError::Failed)
            .and_then(|rt| rt.block_on(users::list_users(&token)));
        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_users_loading(false);
            match result {
                Ok(rows) => {
                    let items: Vec<UserItem> = rows
                        .into_iter()
                        .map(|row| UserItem {
                            id: row.id.into(),
                            username: row.username.into(),
                            class: row.class.unwrap_or_default().into(),
                            parent: row.parent_user_id.unwrap_or_default().into(),
                            active: row.is_active,
                        })
                        .collect();
                    ui.set_users(ModelRc::new(VecModel::from(items)));
                    ui.set_users_error("".into());
                }
                Err(UsersError::Forbidden) => {
                    ui.set_users_error(text(lang, "auth.adminAccessRequired").into())
                }
                Err(UsersError::Failed) => ui.set_users_error(text(lang, "status.failed").into()),
            }
        });
    });
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
    let page_session = session.clone();
    let page_lang = lang.clone();
    let write_lang = lang.clone();
    let write_session = session.clone();
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
                        // Client-side UX only: the Worker remains the authorization boundary.
                        ui.set_is_all_father(new_session.role == "ALL_FATHER");
                        ui.set_page(0);
                        ui.set_signed_in(true);
                        if let Ok(mut slot) = session.lock() {
                            *slot = Some(new_session);
                        }
                        load_users(ui.as_weak(), session.clone(), current_lang);
                    }
                    Err(error) => ui.set_error_text(error_text(current_lang, error).into()),
                }
            });
        });
    });

    let users_weak = ui.as_weak();
    ui.on_select_page(move |page| {
        if page == 0 {
            load_users(users_weak.clone(), page_session.clone(), page_lang.get());
        } else if page == 3 {
            load_orgs(users_weak.clone(), page_session.clone(), page_lang.get());
        }
    });

    let session = sign_out_session;
    let weak = ui.as_weak();
    ui.on_sign_out(move || {
        // Dropping the session clears the in-memory tokens.
        if let Ok(mut slot) = session.lock() {
            *slot = None;
        }
        if let Some(ui) = weak.upgrade() {
            ui.set_signed_in(false);
            ui.set_is_all_father(false);
            ui.set_signed_in_as("".into());
            ui.set_error_text("".into());
        }
    });

    let weak_create = ui.as_weak();
    let session_create = write_session.clone();
    let lang_create = write_lang.clone();
    ui.on_create_user(move |username, email, password, class| {
        let (username, email, password) = (username.to_string(), email.to_string(), password.to_string());
        let class = class.to_string();
        let class = if class.is_empty() { None } else { Some(class) };
        run_user_write(weak_create.clone(), session_create.clone(), lang_create.get(), move |token| async move {
            users::create_user(&token, &username, &email, &password, class.as_deref()).await
        });
    });

    let weak_class = ui.as_weak();
    let session_class = write_session.clone();
    let lang_class = write_lang.clone();
    ui.on_set_class(move |user_id, class| {
        let (user_id, class) = (user_id.to_string(), class.to_string());
        let class = if class.is_empty() { None } else { Some(class) };
        run_user_write(weak_class.clone(), session_class.clone(), lang_class.get(), move |token| async move {
            users::set_class(&token, &user_id, class.as_deref()).await
        });
    });

    let weak_parent = ui.as_weak();
    let session_parent = write_session.clone();
    let lang_parent = write_lang.clone();
    ui.on_set_parent(move |user_id, parent| {
        let (user_id, parent) = (user_id.to_string(), parent.trim().to_string());
        let parent = if parent.is_empty() { None } else { Some(parent) };
        run_user_write(weak_parent.clone(), session_parent.clone(), lang_parent.get(), move |token| async move {
            users::set_parent(&token, &user_id, parent.as_deref()).await
        });
    });

    let weak_active = ui.as_weak();
    let session_active = write_session.clone();
    let lang_active = write_lang.clone();
    ui.on_toggle_active(move |user_id, active| {
        let user_id = user_id.to_string();
        run_user_write(weak_active.clone(), session_active.clone(), lang_active.get(), move |token| async move {
            users::set_active(&token, &user_id, active).await
        });
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_create_org(move |name| {
        let name = name.trim().to_string();
        org_write(weak.clone(), session.clone(), lang.get(), move |token| async move {
            orgs::create_org(&token, &name).await
        }, None);
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_rename_org(move |org_id, name| {
        let (org_id, name) = (org_id.to_string(), name.trim().to_string());
        org_write(weak.clone(), session.clone(), lang.get(), move |token| async move {
            orgs::rename_org(&token, &org_id, &name).await
        }, None);
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_toggle_org(move |org_id, active| {
        let org_id = org_id.to_string();
        org_write(weak.clone(), session.clone(), lang.get(), move |token| async move {
            orgs::set_org_active(&token, &org_id, active).await
        }, None);
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_select_org(move |org_id| {
        load_org_admins(weak.clone(), session.clone(), lang.get(), org_id.to_string());
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_create_org_admin(move |username, email, password| {
        let Some(ui) = weak.upgrade() else { return };
        let org_id = ui.get_selected_org_id().to_string();
        let (username, email, password) = (username.to_string(), email.to_string(), password.to_string());
        org_write(weak.clone(), session.clone(), lang.get(), move |token| async move {
            orgs::create_admin(&token, &org_id, &username, &email, &password).await
        }, Some(ui.get_selected_org_id().to_string()));
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_toggle_admin(move |admin_id, active| {
        let admin_id = admin_id.to_string();
        let org_id = weak.upgrade().map(|ui| ui.get_selected_org_id().to_string());
        org_write(weak.clone(), session.clone(), lang.get(), move |token| async move {
            orgs::set_admin_active(&token, &admin_id, active).await
        }, org_id);
    });

    ui.run()
}
