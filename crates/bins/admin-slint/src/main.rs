//! ONYX Admin native UI (Slint). Phase 1: sign-in with memory-only tokens.

mod auth;
mod i18n;
mod i18n_catalog;
mod id_display;
mod legal_hold;
mod orgs;
mod policy;
mod profiles;
mod settings;
mod users;

use auth::{AdminSession, LoginError};
use i18n::{text, Lang};
use slint::{Model, ModelRc, VecModel};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use users::{ConcurrencyCode, UsersError};

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
    s.set_mobile_heading(text(lang, "nav.settings").into());
    s.set_mobile_save_button(text(lang, "common.save").into());
    s.set_policy_heading(text(lang, "common.policySettings").into());
    s.set_create_policy_button(text(lang, "common.createPolicy").into());
    s.set_policy_name_placeholder(text(lang, "common.policyNameExample").into());
    s.set_publish_button(text(lang, "settings.publishDraft").into());
    s.set_retire_button(text(lang, "settings.retirePolicy").into());
    s.set_evaluate_button(text(lang, "common.evaluate").into());
    s.set_rule_key_placeholder(text(lang, "common.ruleKeyToEvaluate").into());
    s.set_rule_value_placeholder(text(lang, "common.inputValue").into());
    s.set_rule_type_label(text(lang, "common.ruleType").into());
    s.set_add_rule_button(text(lang, "settings.addRule").into());
    s.set_create_version_button(text(lang, "settings.createVersion").into());
    s.set_rules_pending_label(text(lang, "settings.rulesPending").into());
    s.set_legal_holds_heading(text(lang, "common.legalHolds").into());
    s.set_target_id_placeholder(text(lang, "common.targetIdUuid").into());
    s.set_target_type_placeholder(text(lang, "common.targetTypeExample").into());
    s.set_reason_placeholder(text(lang, "common.reason").into());
    s.set_apply_legal_hold_button(text(lang, "settings.applyLegalHold").into());
    s.set_view_hold_placeholder(text(lang, "common.viewHoldById").into());
    s.set_view_button(text(lang, "common.view").into());
    s.set_release_button(text(lang, "settings.release").into());
    s.set_profiles_heading(text(lang, "nav.profiles").into());
    s.set_save_profile_button(text(lang, "common.save").into());
    s.set_export_csv_button(text(lang, "common.exportCsv").into());
    s.set_export_json_button(text(lang, "common.exportJson").into());
    s.set_owner_label(text(lang, "common.userIdUuid").into());
    s.set_full_name_label(text(lang, "common.fullName").into());
    s.set_job_title_label(text(lang, "common.jobTitle").into());
    s.set_contact_email_label(text(lang, "common.contactEmail").into());
    s.set_contact_phone_label(text(lang, "common.contactPhone").into());
    s.set_department_label(text(lang, "common.department").into());
    s.set_team_label(text(lang, "common.teamMemberships").into());
    s.set_import_from_excel(text(lang, "common.importFromExcel").into());
    s.set_show_details(text(lang, "common.showDetails").into());
    s.set_hide_details(text(lang, "common.hideDetails").into());
    s.set_remove_rule(text(lang, "settings.removeRule").into());
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
                Err(error) => show_error(&ui, lang, error),
            }
        });
    });
}

type SessionSlot = Arc<Mutex<Option<AdminSession>>>;
type SelectedPolicy = Arc<Mutex<Option<policy::PolicyRow>>>;
type SelectedHold = Arc<Mutex<Option<legal_hold::HoldRow>>>;
type PendingRules = Arc<Mutex<Vec<serde_json::Value>>>;
type PendingApply = Arc<Mutex<Option<legal_hold::ApplyRequest>>>;

/// Text for the four owner-approved concurrency identifiers. The code is mapped
/// by its stable identifier; message text is never inspected.
fn concurrency_text(lang: Lang, code: ConcurrencyCode) -> &'static str {
    match code {
        ConcurrencyCode::LifecycleEpochRequired => {
            text(lang, "settings.concurrencyLifecycleEpochRequired")
        }
        ConcurrencyCode::AuthorityEpochRequired => {
            text(lang, "settings.concurrencyAuthorityEpochRequired")
        }
        ConcurrencyCode::EpochMismatch => text(lang, "settings.concurrencyEpochMismatch"),
        ConcurrencyCode::VersionConflict => text(lang, "settings.concurrencyVersionConflict"),
    }
}

/// Shared failure text.
fn error_message(lang: Lang, error: UsersError) -> &'static str {
    match error {
        UsersError::Forbidden => text(lang, "auth.adminAccessRequired"),
        UsersError::Failed => text(lang, "status.failed"),
        UsersError::Conflict(code) => concurrency_text(lang, code),
    }
}

fn show_error(ui: &AppWindow, lang: Lang, error: UsersError) {
    ui.set_users_error(error_message(lang, error).into());
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
        let selected = ui.get_orgs().iter().find(|org| org.id.as_str() == org_id);
        let name = selected
            .as_ref()
            .map(|org| org.name.to_string())
            .unwrap_or_default();
        let active = selected.as_ref().is_some_and(|org| org.active);
        ui.set_selected_org_id(org_id.clone().into());
        ui.set_selected_org_name(name.into());
        ui.set_selected_org_active(active);
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
    run_org_call(
        weak,
        session,
        lang,
        action,
        move |weak, session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(()) => match admins_of {
                    Some(org_id) => load_org_admins(weak.clone(), session, lang, org_id),
                    None => load_orgs(weak.clone(), session, lang),
                },
                Err(error) => show_error(&ui, lang, error),
            }
        },
    );
}

/// Loads the mobile-access policy into the Settings page.
fn load_mobile_access(weak: slint::Weak<AppWindow>, session: SessionSlot, lang: Lang) {
    run_org_call(
        weak,
        session,
        lang,
        |token| async move { settings::get_mobile_access(&token).await },
        move |weak, _session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(policy) => apply_mobile_access(&ui, &policy),
                Err(error) => show_error(&ui, lang, error),
            }
        },
    );
}

fn apply_mobile_access(ui: &AppWindow, policy: &settings::MobileAccess) {
    let has = |class: &str| policy.allowed_classes.iter().any(|c| c == class);
    ui.set_mobile_top(has("top_level_manager"));
    ui.set_mobile_senior(has("senior_manager"));
    ui.set_mobile_team(has("team_leader"));
    ui.set_mobile_supervisor(has("supervisor"));
    ui.set_mobile_staff(has("staff"));
    ui.set_mobile_version(policy.version as i32);
    ui.set_mobile_status(policy.enforcement_status.clone().into());
}

/// Organization of the signed-in admin; policy calls are scoped to it.
fn org_id_of(session: &SessionSlot) -> Option<String> {
    session
        .lock()
        .ok()?
        .as_ref()
        .map(|s| s.organization_id.clone())
}

fn show_policy_error(ui: &AppWindow, lang: Lang, error: UsersError) {
    ui.set_policy_error(error_message(lang, error).into());
}

/// Loads the policy list for the Settings page.
fn load_policies(weak: slint::Weak<AppWindow>, session: SessionSlot, lang: Lang) {
    let Some(org) = org_id_of(&session) else {
        return;
    };
    run_org_call(
        weak,
        session,
        lang,
        move |token| async move { policy::list_policies(&token, &org).await },
        move |weak, _session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(rows) => {
                    let items: Vec<PolicyItem> = rows
                        .iter()
                        .map(|row| PolicyItem {
                            id: row.id.clone().into(),
                            name: row.name.clone().into(),
                            status: row.status.clone().into(),
                            version: row.version as i32,
                        })
                        .collect();
                    ui.set_policies(ModelRc::new(VecModel::from(items)));
                }
                Err(error) => show_policy_error(&ui, lang, error),
            }
        },
    );
}

/// Reads one policy into the detail panel and keeps its concurrency values for the next command.
fn load_policy_detail(
    weak: slint::Weak<AppWindow>,
    session: SessionSlot,
    lang: Lang,
    id: String,
    selected: SelectedPolicy,
) {
    let Some(org) = org_id_of(&session) else {
        return;
    };
    run_org_call(
        weak,
        session,
        lang,
        move |token| async move { policy::policy_detail(&token, &org, &id).await },
        move |weak, _session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(policy::PolicyDetail::Found(row)) => {
                    ui.set_selected_policy_id(row.id.clone().into());
                    ui.set_selected_policy_name(row.name.clone().into());
                    ui.set_selected_policy_status(row.status.clone().into());
                    ui.set_selected_policy_version(row.version as i32);
                    if let Ok(mut slot) = selected.lock() {
                        *slot = Some(row);
                    }
                }
                Ok(policy::PolicyDetail::NotFound) => {
                    // Drop the old selection so no command can target a stale policy.
                    if let Ok(mut slot) = selected.lock() {
                        *slot = None;
                    }
                    ui.set_selected_policy_id("".into());
                    ui.set_policy_error(text(lang, "common.noPolicyFound").into());
                }
                Ok(policy::PolicyDetail::MissingConcurrency) => {
                    if let Ok(mut slot) = selected.lock() {
                        *slot = None;
                    }
                    ui.set_selected_policy_id("".into());
                    ui.set_policy_error(
                        text(lang, "settings.policyMissingConcurrencyEpochs").into(),
                    );
                }
                Err(error) => show_policy_error(&ui, lang, error),
            }
        },
    );
}

/// Rebuilds the pending-rule list shown on the Settings page.
fn refresh_pending_rules(ui: &AppWindow, pending: &PendingRules) {
    let rules = pending
        .lock()
        .map(|rules| rules.clone())
        .unwrap_or_default();
    let items: Vec<PendingRuleItem> = rules
        .iter()
        .map(|rule| PendingRuleItem {
            rule_type: rule
                .get("rule_type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .into(),
            key: rule
                .get("key")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .into(),
            value: policy::rule_value_text(rule.get("value").unwrap_or(&serde_json::Value::Null))
                .into(),
        })
        .collect();
    ui.set_rules_pending(items.len() as i32);
    ui.set_pending_rules(ModelRc::new(VecModel::from(items)));
}

/// Sends one lifecycle command to the selected policy, then reloads the list and detail.
fn policy_command(
    weak: slint::Weak<AppWindow>,
    session: SessionSlot,
    lang: Lang,
    selected: SelectedPolicy,
    command: &'static str,
    payload: serde_json::Value,
    pending: PendingRules,
) {
    let Some(org) = org_id_of(&session) else {
        return;
    };
    let Some(row) = selected.lock().ok().and_then(|slot| slot.clone()) else {
        return;
    };
    let pending_done = pending.clone();
    let id = row.id.clone();
    let concurrency = row.concurrency();
    let reload = selected.clone();
    if let Some(ui) = weak.upgrade() {
        ui.set_policy_command_busy(true);
    }
    run_org_call(
        weak,
        session,
        lang,
        move |token| async move {
            policy::send_command(&token, &org, "policy", command, &concurrency, payload).await
        },
        move |weak, session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            ui.set_policy_command_busy(false);
            match result {
                Ok(response) => {
                    if command == "policy.CreatePolicyVersion" {
                        if let Ok(mut rules) = pending_done.lock() {
                            rules.clear();
                        }
                        refresh_pending_rules(&ui, &pending_done);
                    }
                    if command == "policy.EvaluatePolicy" {
                        match policy::evaluation_view(lang, &response) {
                            Some(view) => {
                                ui.set_policy_status(view.summary.into());
                                ui.set_policy_evaluation_detail(view.detail.into());
                                ui.set_policy_evaluation_detail_visible(false);
                            }
                            None => {
                                ui.set_policy_status("".into());
                                ui.set_policy_evaluation_detail("".into());
                                ui.set_policy_evaluation_detail_visible(false);
                            }
                        }
                    } else {
                        ui.set_policy_status("".into());
                        ui.set_policy_evaluation_detail("".into());
                        ui.set_policy_evaluation_detail_visible(false);
                    }
                    load_policies(weak.clone(), session.clone(), lang);
                    load_policy_detail(weak, session, lang, id, reload);
                }
                Err(error) => show_policy_error(&ui, lang, error),
            }
        },
    );
}

fn show_hold_error(ui: &AppWindow, lang: Lang, error: UsersError) {
    ui.set_legal_hold_error(error_message(lang, error).into());
}

/// Reads one hold into the Settings panel and keeps its concurrency values for release.
fn load_hold_detail(
    weak: slint::Weak<AppWindow>,
    session: SessionSlot,
    lang: Lang,
    id: String,
    selected: SelectedHold,
) {
    let Some(org) = org_id_of(&session) else {
        return;
    };
    run_org_call(
        weak,
        session,
        lang,
        move |token| async move { legal_hold::hold_detail(&token, &org, &id).await },
        move |weak, _session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(legal_hold::HoldDetail::Found(hold)) => {
                    ui.set_legal_hold_selected_id(hold.id().to_string().into());
                    ui.set_legal_hold_state(format!("Status: {}", hold.status).into());
                    ui.set_legal_hold_can_release(hold.status == "Applied");
                    if let Ok(mut slot) = selected.lock() {
                        *slot = Some(hold);
                    }
                }
                Ok(legal_hold::HoldDetail::NotFound) => {
                    // Drop the old selection so no release can target a stale hold.
                    if let Ok(mut slot) = selected.lock() {
                        *slot = None;
                    }
                    ui.set_legal_hold_selected_id("".into());
                    ui.set_legal_hold_can_release(false);
                    ui.set_legal_hold_error(text(lang, "settings.noLegalHoldFound").into());
                }
                Ok(legal_hold::HoldDetail::MissingConcurrency) => {
                    if let Ok(mut slot) = selected.lock() {
                        *slot = None;
                    }
                    ui.set_legal_hold_selected_id("".into());
                    ui.set_legal_hold_can_release(false);
                    ui.set_legal_hold_error(
                        text(lang, "settings.legalHoldMissingConcurrencyEpochs").into(),
                    );
                }
                Err(error) => show_hold_error(&ui, lang, error),
            }
        },
    );
}

/// Loads the legal-hold list for the Settings page.
fn load_holds(weak: slint::Weak<AppWindow>, session: SessionSlot, lang: Lang) {
    let Some(org) = org_id_of(&session) else {
        return;
    };
    run_org_call(
        weak,
        session,
        lang,
        move |token| async move { legal_hold::list_holds(&token, &org).await },
        move |weak, _session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(rows) => {
                    let items: Vec<HoldItem> = rows
                        .into_iter()
                        .map(|hold| HoldItem {
                            display_id: id_display::display_id(&hold.id).into(),
                            id: hold.id.into(),
                            status: hold.status.into(),
                        })
                        .collect();
                    ui.set_legal_holds(ModelRc::new(VecModel::from(items)));
                }
                Err(error) => show_hold_error(&ui, lang, error),
            }
        },
    );
}

/// Loads the staff profile list.
fn load_profiles(weak: slint::Weak<AppWindow>, session: SessionSlot, lang: Lang) {
    run_org_call(
        weak,
        session,
        lang,
        |token| async move { profiles::list_profiles(&token).await },
        move |weak, _session, lang, result| {
            let Some(ui) = weak.upgrade() else { return };
            match result {
                Ok(rows) => {
                    let items: Vec<ProfileItem> = rows
                        .into_iter()
                        .map(|row| ProfileItem {
                            owner: row.owner_id.into(),
                            name: row.full_name.into(),
                            title: row.job_title.unwrap_or_default().into(),
                            department: row.department.unwrap_or_default().into(),
                            class: row.class_label.unwrap_or_default().into(),
                            email: row.contact_email.unwrap_or_default().into(),
                            phone: row.contact_phone.unwrap_or_default().into(),
                            parent: row.parent_display_name.unwrap_or_default().into(),
                            teams: row.team_memberships.join("; ").into(),
                            photo: row.photo_blob_key.unwrap_or_default().into(),
                        })
                        .collect();
                    ui.set_profiles(ModelRc::new(VecModel::from(items)));
                }
                Err(error) => show_error(&ui, lang, error),
            }
        },
    );
}

/// Turns an optional form field into `None` when blank.
fn opt(value: slint::SharedString) -> Option<String> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
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
                Err(error) => {
                    ui.set_users_error(error_message(lang, error).into());
                }
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
        } else if page == 1 {
            load_mobile_access(users_weak.clone(), page_session.clone(), page_lang.get());
            load_policies(users_weak.clone(), page_session.clone(), page_lang.get());
            load_holds(users_weak.clone(), page_session.clone(), page_lang.get());
        } else if page == 2 {
            load_profiles(users_weak.clone(), page_session.clone(), page_lang.get());
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
        let (username, email, password) = (
            username.to_string(),
            email.to_string(),
            password.to_string(),
        );
        let class = class.to_string();
        let class = if class.is_empty() { None } else { Some(class) };
        run_user_write(
            weak_create.clone(),
            session_create.clone(),
            lang_create.get(),
            move |token| async move {
                users::create_user(&token, &username, &email, &password, class.as_deref()).await
            },
        );
    });

    let weak_class = ui.as_weak();
    let session_class = write_session.clone();
    let lang_class = write_lang.clone();
    ui.on_set_class(move |user_id, class| {
        let (user_id, class) = (user_id.to_string(), class.to_string());
        let class = if class.is_empty() { None } else { Some(class) };
        run_user_write(
            weak_class.clone(),
            session_class.clone(),
            lang_class.get(),
            move |token| async move { users::set_class(&token, &user_id, class.as_deref()).await },
        );
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
        run_user_write(
            weak_active.clone(),
            session_active.clone(),
            lang_active.get(),
            move |token| async move { users::set_active(&token, &user_id, active).await },
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_create_org(move |name| {
        let name = name.trim().to_string();
        org_write(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move { orgs::create_org(&token, &name).await },
            None,
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_rename_org(move |org_id, name| {
        let (org_id, name) = (org_id.to_string(), name.trim().to_string());
        org_write(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move { orgs::rename_org(&token, &org_id, &name).await },
            None,
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_toggle_org(move |org_id, active| {
        let org_id = org_id.to_string();
        org_write(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move { orgs::set_org_active(&token, &org_id, active).await },
            None,
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_select_org(move |org_id| {
        load_org_admins(
            weak.clone(),
            session.clone(),
            lang.get(),
            org_id.to_string(),
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_create_org_admin(move |username, email, password| {
        let Some(ui) = weak.upgrade() else { return };
        let org_id = ui.get_selected_org_id().to_string();
        let (username, email, password) = (
            username.to_string(),
            email.to_string(),
            password.to_string(),
        );
        org_write(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move {
                orgs::create_admin(&token, &org_id, &username, &email, &password).await
            },
            Some(ui.get_selected_org_id().to_string()),
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_toggle_admin(move |admin_id, active| {
        let admin_id = admin_id.to_string();
        let org_id = weak
            .upgrade()
            .map(|ui| ui.get_selected_org_id().to_string());
        org_write(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move { orgs::set_admin_active(&token, &admin_id, active).await },
            org_id,
        );
    });

    let selected_policy: SelectedPolicy = Arc::new(Mutex::new(None));
    let pending_rules: PendingRules = Arc::new(Mutex::new(Vec::new()));
    let (weak, session, lang, selected, pending) = (
        ui.as_weak(),
        write_session.clone(),
        write_lang.clone(),
        selected_policy.clone(),
        pending_rules.clone(),
    );
    ui.on_select_policy(move |id| {
        // Pending rules belong to the policy that was being edited. Switching
        // policies starts a new draft instead of leaking rules across policies.
        if let Ok(mut rules) = pending.lock() {
            rules.clear();
        }
        if let Some(ui) = weak.upgrade() {
            refresh_pending_rules(&ui, &pending);
            ui.set_policy_status("".into());
            ui.set_policy_evaluation_detail("".into());
            ui.set_policy_evaluation_detail_visible(false);
        }
        load_policy_detail(
            weak.clone(),
            session.clone(),
            lang.get(),
            id.to_string(),
            selected.clone(),
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_create_policy(move |name| {
        let name = name.to_string();
        let key = new_request_id();
        run_org_call(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move { policy::create_policy(&token, &name, &key).await.map(|_| ()) },
            move |weak, session, lang, result| match result {
                Ok(()) => load_policies(weak, session, lang),
                Err(error) => {
                    if let Some(ui) = weak.upgrade() {
                        show_policy_error(&ui, lang, error);
                    }
                }
            },
        );
    });

    let (weak, session, lang, selected, pending) = (
        ui.as_weak(),
        write_session.clone(),
        write_lang.clone(),
        selected_policy.clone(),
        pending_rules.clone(),
    );
    ui.on_publish_policy(move || {
        policy_command(
            weak.clone(),
            session.clone(),
            lang.get(),
            selected.clone(),
            "policy.PublishPolicyVersion",
            serde_json::json!({}),
            pending.clone(),
        )
    });

    let (weak, session, lang, selected, pending) = (
        ui.as_weak(),
        write_session.clone(),
        write_lang.clone(),
        selected_policy.clone(),
        pending_rules.clone(),
    );
    ui.on_retire_policy(move || {
        policy_command(
            weak.clone(),
            session.clone(),
            lang.get(),
            selected.clone(),
            "policy.RetirePolicy",
            serde_json::json!({}),
            pending.clone(),
        )
    });

    let (weak, pending) = (ui.as_weak(), pending_rules.clone());
    ui.on_add_policy_rule(move |rule_type, key, value| {
        let key = key.trim().to_string();
        if key.is_empty() {
            return;
        }
        let Some(ui) = weak.upgrade() else { return };
        {
            let Ok(mut rules) = pending.lock() else {
                return;
            };
            rules.push(serde_json::json!({
                "rule_type": rule_type.to_string(),
                "key": key,
                "value": policy::parse_rule_value(value.trim()),
            }));
        }
        refresh_pending_rules(&ui, &pending);
        ui.set_rule_key_input("".into());
        ui.set_rule_value_input("".into());
    });

    let (weak, pending) = (ui.as_weak(), pending_rules.clone());
    ui.on_remove_policy_rule(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        if let Ok(mut rules) = pending.lock() {
            let index = index as usize;
            if index < rules.len() {
                rules.remove(index);
            }
        }
        refresh_pending_rules(&ui, &pending);
    });

    let weak = ui.as_weak();
    ui.on_toggle_policy_evaluation_detail(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_policy_evaluation_detail_visible(!ui.get_policy_evaluation_detail_visible());
        }
    });

    let (weak, session, lang, selected, pending) = (
        ui.as_weak(),
        write_session.clone(),
        write_lang.clone(),
        selected_policy.clone(),
        pending_rules.clone(),
    );
    ui.on_create_policy_version(move || {
        let rules = pending
            .lock()
            .map(|rules| rules.clone())
            .unwrap_or_default();
        if rules.is_empty() {
            return;
        }
        policy_command(
            weak.clone(),
            session.clone(),
            lang.get(),
            selected.clone(),
            "policy.CreatePolicyVersion",
            serde_json::json!({ "rules": rules }),
            pending.clone(),
        );
    });

    let (weak, session, lang, selected, pending) = (
        ui.as_weak(),
        write_session.clone(),
        write_lang.clone(),
        selected_policy.clone(),
        pending_rules.clone(),
    );
    ui.on_evaluate_policy(move |key| {
        policy_command(
            weak.clone(),
            session.clone(),
            lang.get(),
            selected.clone(),
            "policy.EvaluatePolicy",
            serde_json::json!({ "rule_key": key.to_string() }),
            pending.clone(),
        )
    });

    let selected_hold: SelectedHold = Arc::new(Mutex::new(None));
    let pending_apply: PendingApply = Arc::new(Mutex::new(None));
    let (weak, session, lang, selected) = (
        ui.as_weak(),
        write_session.clone(),
        write_lang.clone(),
        selected_hold.clone(),
    );
    ui.on_view_legal_hold(move |id| {
        load_hold_detail(
            weak.clone(),
            session.clone(),
            lang.get(),
            id.to_string(),
            selected.clone(),
        );
    });

    let (weak, session, lang, selected) = (
        ui.as_weak(),
        write_session.clone(),
        write_lang.clone(),
        selected_hold.clone(),
    );
    ui.on_select_legal_hold(move |id| {
        load_hold_detail(
            weak.clone(),
            session.clone(),
            lang.get(),
            id.to_string(),
            selected.clone(),
        );
    });

    let (weak, session, lang, selected, pending_apply) = (
        ui.as_weak(),
        write_session.clone(),
        write_lang.clone(),
        selected_hold.clone(),
        pending_apply.clone(),
    );
    ui.on_apply_legal_hold(move |target_id, target_type, reason| {
        let (target_id, target_type, reason) = (
            target_id.trim().to_string(),
            target_type.trim().to_string(),
            reason.trim().to_string(),
        );
        if target_id.is_empty() || target_type.is_empty() || reason.is_empty() {
            return;
        }
        // The key is kept across retries of this exact action and cleared only on
        // success, so an uncertain failure cannot create a duplicate hold.
        let request = {
            let previous = pending_apply.lock().ok().and_then(|slot| slot.clone());
            legal_hold::prepare_apply(
                previous.as_ref(),
                &target_id,
                &target_type,
                &reason,
                new_request_id,
            )
        };
        if let Ok(mut slot) = pending_apply.lock() {
            *slot = Some(request.clone());
        }
        let key = request.idempotency_key.clone();
        let selected = selected.clone();
        let pending_done = pending_apply.clone();
        run_org_call(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move {
                legal_hold::apply(&token, &target_id, &target_type, &reason, &key).await
            },
            move |weak, session, lang, result| {
                let Some(ui) = weak.upgrade() else { return };
                match result {
                    Ok(hold_id) => {
                        if let Ok(mut slot) = pending_done.lock() {
                            *slot = None;
                        }
                        ui.set_legal_hold_target_input("".into());
                        ui.set_legal_hold_reason_input("".into());
                        ui.set_legal_hold_view_id(hold_id.clone().into());
                        load_hold_detail(weak.clone(), session.clone(), lang, hold_id, selected);
                        load_holds(weak, session, lang);
                    }
                    Err(error) => show_hold_error(&ui, lang, error),
                }
            },
        );
    });

    let (weak, session, lang, selected) = (
        ui.as_weak(),
        write_session.clone(),
        write_lang.clone(),
        selected_hold.clone(),
    );
    ui.on_release_legal_hold(move || {
        let Some(ui) = weak.upgrade() else { return };
        let reason = ui.get_legal_hold_release_reason_input().trim().to_string();
        if reason.is_empty() {
            return;
        }
        let Some(org) = org_id_of(&session) else {
            return;
        };
        let Some(hold) = selected.lock().ok().and_then(|slot| slot.clone()) else {
            return;
        };
        let id = hold.concurrency.id.clone();
        let reload = selected.clone();
        run_org_call(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move { legal_hold::release(&token, &org, &hold, &reason).await },
            move |weak, session, lang, result| {
                let Some(ui) = weak.upgrade() else { return };
                match result {
                    Ok(()) => {
                        ui.set_legal_hold_release_reason_input("".into());
                        load_hold_detail(weak.clone(), session.clone(), lang, id, reload);
                        load_holds(weak, session, lang);
                    }
                    Err(error) => show_hold_error(&ui, lang, error),
                }
            },
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_save_mobile_access(move |top, senior, team, supervisor, staff| {
        let Some(ui) = weak.upgrade() else { return };
        let version = i64::from(ui.get_mobile_version());
        let mut classes = Vec::new();
        for (on, class) in [
            (top, "top_level_manager"),
            (senior, "senior_manager"),
            (team, "team_leader"),
            (supervisor, "supervisor"),
            (staff, "staff"),
        ] {
            if on {
                classes.push(class.to_string());
            }
        }
        run_org_call(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move { settings::put_mobile_access(&token, &classes, version).await },
            move |weak, _session, lang, result| {
                let Some(ui) = weak.upgrade() else { return };
                match result {
                    Ok(policy) => apply_mobile_access(&ui, &policy),
                    Err(error) => show_error(&ui, lang, error),
                }
            },
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_save_profile(move || {
        let Some(ui) = weak.upgrade() else { return };
        let team_memberships: Vec<String> = ui
            .get_form_teams()
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let form = profiles::ProfileForm {
            owner_id: ui.get_form_owner().trim().to_string(),
            full_name: ui.get_form_name().trim().to_string(),
            job_title: opt(ui.get_form_title()),
            contact_email: opt(ui.get_form_email()),
            contact_phone: opt(ui.get_form_phone()),
            department: opt(ui.get_form_department()),
            photo_blob_key: opt(ui.get_form_photo()),
            class_label: opt(ui.get_form_class()),
            parent_display_name: opt(ui.get_form_parent()),
            team_memberships,
        };
        run_org_call(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move { profiles::save_profile(&token, &form).await },
            move |weak, session, lang, result| match result {
                Ok(()) => load_profiles(weak, session, lang),
                Err(error) => {
                    if let Some(ui) = weak.upgrade() {
                        show_error(&ui, lang, error);
                    }
                }
            },
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_import_profiles(move || {
        run_org_call(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move {
                let picked = tokio::task::spawn_blocking(profiles::pick_import_file)
                    .await
                    .map_err(|_| UsersError::Failed)?;
                let Some((bytes, content_type)) = picked else {
                    return Ok(None);
                };
                let report = profiles::import_bytes(&token, bytes, content_type).await?;
                // The report holds one-time passwords. It is saved only where the user chooses.
                let summary = format!("{} / {} / {}", report.total, report.created, report.failed);
                let saved = tokio::task::spawn_blocking(move || {
                    profiles::save_file(&report.bytes, "onyx-creation-report.xlsx")
                })
                .await
                .map_err(|_| UsersError::Failed)??;
                Ok(saved.then_some(summary))
            },
            move |weak, session, lang, result| match result {
                Ok(Some(summary)) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_import_status(summary.into());
                    }
                    load_profiles(weak, session, lang)
                }
                Ok(None) => {}
                Err(error) => {
                    if let Some(ui) = weak.upgrade() {
                        show_error(&ui, lang, error);
                    }
                }
            },
        );
    });

    let (weak, session, lang) = (ui.as_weak(), write_session.clone(), write_lang.clone());
    ui.on_export_profiles(move |format| {
        let format = format.to_string();
        run_org_call(
            weak.clone(),
            session.clone(),
            lang.get(),
            move |token| async move {
                let bytes = profiles::export_bytes(&token, &format).await?;
                let file_name = format!("profiles.{format}");
                tokio::task::spawn_blocking(move || profiles::save_file(&bytes, &file_name))
                    .await
                    .map_err(|_| UsersError::Failed)?
                    .map(|_| ())
            },
            move |weak, _session, lang, result| {
                if let (Some(ui), Err(error)) = (weak.upgrade(), result) {
                    show_error(&ui, lang, error);
                }
            },
        );
    });

    ui.run()
}
