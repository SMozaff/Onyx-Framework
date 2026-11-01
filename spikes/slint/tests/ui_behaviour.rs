//! Headless UI checks for U-01 (tables, multiline, Persian and mixed-direction input)
//! and U-02 (accessible labels and roles in the Slint element tree).
//! Uses the Slint testing backend, so no display is needed.
//! Each test binary may set the platform only once, so all checks are in one test.

use i_slint_backend_testing::{ElementHandle, ElementQuery};
use onyx_slint_spike::{create_app, AppWindow, Motion, Theme};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, SharedString};

fn labels_with_prefix(app: &AppWindow, prefix: &'static str) -> Vec<String> {
    ElementQuery::from_root(app)
        .match_predicate(move |e| e.accessible_label().is_some_and(|l| l.starts_with(prefix)))
        .find_all()
        .into_iter()
        .map(|e| e.accessible_label().unwrap().to_string())
        .collect()
}

fn click(app: &AppWindow, label: &str) {
    let el = ElementHandle::find_by_accessible_label(app, label)
        .next()
        .unwrap_or_else(|| panic!("no element with accessible label {label:?}"));
    el.mock_single_click(PointerEventButton::Left);
}

fn names(app: &AppWindow, prefix: &'static str) -> Vec<String> {
    labels_with_prefix(app, prefix)
        .into_iter()
        .map(|l| l.split_once(": ").unwrap().1.to_string())
        .collect()
}

fn type_text(app: &AppWindow, text: &str) {
    let window = app.window();
    for ch in text.chars() {
        let t = SharedString::from(ch.to_string().as_str());
        window.dispatch_event(WindowEvent::KeyPressed { text: t.clone() });
        window.dispatch_event(WindowEvent::KeyReleased { text: t });
    }
}

#[test]
fn ui_behaviour_checks() {
    i_slint_backend_testing::init_no_event_loop();
    let app = create_app().expect("app should build");
    app.show().expect("window should show");

    // ---- U-01.1 sortable table ----
    let initial = names(&app, "cell-name:");
    assert_eq!(initial.len(), 10, "all sample rows render");
    assert_eq!(initial[0], "Maryam", "unsorted order is the sample order");

    click(&app, "Sort by name");
    let asc = names(&app, "cell-name:");
    let mut expected = initial.clone();
    expected.sort_by_key(|n| n.to_lowercase());
    assert_eq!(asc, expected, "name ascending after first click");

    click(&app, "Sort by name");
    let mut desc = expected.clone();
    desc.reverse();
    assert_eq!(
        names(&app, "cell-name:"),
        desc,
        "name descending after second click"
    );

    click(&app, "Sort by score");
    let scores: Vec<i32> = labels_with_prefix(&app, "cell-score:")
        .iter()
        .map(|l| l.split_once(": ").unwrap().1.parse().unwrap())
        .collect();
    assert_eq!(scores.len(), 10);
    assert!(
        scores.windows(2).all(|w| w[0] <= w[1]),
        "score ascending: {scores:?}"
    );
    assert_eq!(
        app.get_sort_label().to_string(),
        "Sorted by Score, ascending"
    );

    // ---- U-01.2 paged table ----
    assert_eq!(app.get_page_label().to_string(), "Page 1 of 3");
    assert_eq!(names(&app, "page-cell-name:").len(), 4);
    assert!(!app.get_can_prev(), "Previous disabled on first page");
    assert!(app.get_can_next());
    let prev = ElementHandle::find_by_accessible_label(&app, "Previous page")
        .next()
        .unwrap();
    assert_eq!(
        prev.accessible_enabled(),
        Some(false),
        "Previous button disabled on page 1"
    );

    click(&app, "Next page");
    assert_eq!(app.get_page_label().to_string(), "Page 2 of 3");
    assert_eq!(
        names(&app, "page-cell-name:"),
        vec!["Chloe", "Dariush", "Elham", "Farhad"]
    );

    click(&app, "Next page");
    assert_eq!(app.get_page_label().to_string(), "Page 3 of 3");
    assert_eq!(
        names(&app, "page-cell-name:"),
        vec!["گلناز", "Hossein"],
        "last page has 2 rows"
    );
    assert!(!app.get_can_next(), "Next disabled on last page");

    click(&app, "Previous page");
    assert_eq!(app.get_page_label().to_string(), "Page 2 of 3");

    // ---- U-01.3 multiline: Enter inserts a newline ----
    app.invoke_focus_multiline();
    type_text(&app, "ab");
    type_text(&app, "\n");
    type_text(&app, "cd");
    let multiline = app.get_multiline_text().to_string();
    println!("multiline value after typing: {multiline:?}");
    assert_eq!(multiline, "ab\ncd", "Enter inserts a newline in TextEdit");

    // ---- U-01.4 Persian keystrokes (simulated keys, not an IME) ----
    app.invoke_focus_persian();
    type_text(&app, "سلام دنیا");
    assert_eq!(
        app.get_persian_text().to_string(),
        "سلام دنیا",
        "Persian keystrokes stored"
    );

    // ---- U-01.5 mixed direction keystrokes ----
    app.invoke_focus_bidi();
    type_text(&app, "Hello سلام world");
    assert_eq!(
        app.get_bidi_text().to_string(),
        "Hello سلام world",
        "mixed text stored"
    );

    // ---- U-02.1 labels and roles ----
    let button_labels = [
        "Sort by name",
        "Sort by role",
        "Sort by score",
        "Previous page",
        "Next page",
        "Toggle theme",
    ];
    for label in button_labels {
        let el = ElementHandle::find_by_accessible_label(&app, label)
            .next()
            .unwrap_or_else(|| panic!("missing label {label:?}"));
        assert_eq!(
            el.accessible_role(),
            Some(i_slint_backend_testing::AccessibleRole::Button),
            "role of {label:?}"
        );
    }
    for label in ["Multiline notes", "Persian input", "Mixed direction input"] {
        let el = ElementHandle::find_by_accessible_label(&app, label)
            .next()
            .unwrap_or_else(|| panic!("missing label {label:?}"));
        assert_eq!(
            el.accessible_role(),
            Some(i_slint_backend_testing::AccessibleRole::TextInput),
            "role of {label:?}"
        );
    }

    // ---- Theme toggle (U-02.4 precondition: both themes reachable) ----
    assert!(!app.global::<Theme>().get_dark());
    click(&app, "Toggle theme");
    assert!(
        app.global::<Theme>().get_dark(),
        "dark theme on after toggle"
    );
    click(&app, "Toggle theme");
    assert!(
        !app.global::<Theme>().get_dark(),
        "light theme on after second toggle"
    );

    // ---- U-10 flag reaches Slint ----
    app.global::<Motion>().set_reduced(false);
    assert!(!app.global::<Motion>().get_reduced());
    app.global::<Motion>().set_reduced(true);
    assert!(app.global::<Motion>().get_reduced());
}
