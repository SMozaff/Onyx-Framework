//! ONYX Slint feasibility spike (brief B-13: U-01, U-02, U-03, U-10).
//! Test data only. Not product code.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

pub mod logic;

slint::include_modules!();

pub const PAGE_SIZE: usize = 4;

fn to_row(p: &logic::Person) -> Row {
    Row {
        name: SharedString::from(p.name.as_str()),
        role: SharedString::from(p.role.as_str()),
        score: SharedString::from(p.score.to_string().as_str()),
    }
}

fn sort_label(state: Option<logic::SortState>) -> String {
    match state {
        None => "Sort: none".to_string(),
        Some(s) => format!(
            "Sorted by {:?}, {}",
            s.key,
            if s.ascending {
                "ascending"
            } else {
                "descending"
            }
        ),
    }
}

/// Shared spike state. Rust owns sort and paging; Slint only renders what is set here.
struct State {
    people: Vec<logic::Person>,
    sort: Option<logic::SortState>,
    page: usize,
}

fn refresh(app: &AppWindow, st: &State) {
    let mut sorted = st.people.clone();
    if let Some(s) = st.sort {
        logic::sort_people(&mut sorted, s);
    }
    let rows: Vec<Row> = sorted.iter().map(to_row).collect();
    app.set_sort_rows(ModelRc::new(VecModel::from(rows)));
    app.set_sort_label(SharedString::from(sort_label(st.sort).as_str()));

    let total_pages = logic::page_count(st.people.len(), PAGE_SIZE);
    let page = st.page.min(total_pages - 1);
    let page_rows: Vec<Row> = logic::page_slice(&st.people, page, PAGE_SIZE)
        .iter()
        .map(to_row)
        .collect();
    app.set_page_rows(ModelRc::new(VecModel::from(page_rows)));
    app.set_page_label(SharedString::from(
        format!("Page {} of {}", page + 1, total_pages).as_str(),
    ));
    app.set_can_prev(page > 0);
    app.set_can_next(page + 1 < total_pages);
}

fn motion_label(os: Option<bool>, pref: logic::MotionPref) -> String {
    let os_text = match os {
        Some(true) => "enabled",
        Some(false) => "disabled",
        None => "unknown or unreadable",
    };
    format!(
        "OS animations: {os_text}. Applied: {}",
        match pref {
            logic::MotionPref::Full => "full motion",
            logic::MotionPref::Reduced => "reduced motion",
        }
    )
}

/// Builds the window with test data, wires callbacks, and applies the OS motion setting.
pub fn create_app() -> Result<AppWindow, slint::PlatformError> {
    let app = AppWindow::new()?;

    let os = logic::read_os_animations();
    let pref = logic::resolve_motion(os);
    app.global::<Motion>()
        .set_reduced(pref == logic::MotionPref::Reduced);
    app.set_motion_label(SharedString::from(motion_label(os, pref).as_str()));

    let state = Rc::new(RefCell::new(State {
        people: logic::sample_people(),
        sort: None,
        page: 0,
    }));
    refresh(&app, &state.borrow());

    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_sort_requested(move |key| {
            let Some(key) = logic::SortKey::parse(&key) else {
                return;
            };
            let mut st = state.borrow_mut();
            st.sort = Some(logic::SortState::next_for(st.sort, key));
            if let Some(app) = weak.upgrade() {
                refresh(&app, &st);
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_next_page(move || {
            let mut st = state.borrow_mut();
            st.page += 1;
            if let Some(app) = weak.upgrade() {
                refresh(&app, &st);
            }
        });
    }
    {
        let weak = app.as_weak();
        let state = state.clone();
        app.on_prev_page(move || {
            let mut st = state.borrow_mut();
            st.page = st.page.saturating_sub(1);
            if let Some(app) = weak.upgrade() {
                refresh(&app, &st);
            }
        });
    }
    {
        let weak = app.as_weak();
        app.on_toggle_theme(move || {
            if let Some(app) = weak.upgrade() {
                let dark = app.global::<Theme>().get_dark();
                app.global::<Theme>().set_dark(!dark);
            }
        });
    }
    Ok(app)
}

/// Text printed by `--motion-report`: what the OS reported and what Slint's global holds.
pub fn motion_report() -> Result<String, slint::PlatformError> {
    let app = AppWindow::new()?;
    let os = logic::read_os_animations();
    let pref = logic::resolve_motion(os);
    app.global::<Motion>()
        .set_reduced(pref == logic::MotionPref::Reduced);
    let slint_reduced = app.global::<Motion>().get_reduced();
    Ok(format!(
        "{{\"os_animations\": {}, \"resolved\": \"{}\", \"slint_reduced\": {}}}",
        match os {
            Some(b) => b.to_string(),
            None => "null".to_string(),
        },
        match pref {
            logic::MotionPref::Full => "full",
            logic::MotionPref::Reduced => "reduced",
        },
        slint_reduced
    ))
}
