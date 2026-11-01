//! Pure logic for the spike: sorting, paging, and motion-preference resolution.
//! Everything here is testable without a display.

use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub role: String,
    pub score: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Role,
    Score,
}

impl SortKey {
    pub fn parse(s: &str) -> Option<SortKey> {
        match s {
            "name" => Some(SortKey::Name),
            "role" => Some(SortKey::Role),
            "score" => Some(SortKey::Score),
            _ => None,
        }
    }
}

/// Sort state for the sortable table. Clicking the same header again flips direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortState {
    pub key: SortKey,
    pub ascending: bool,
}

impl SortState {
    pub fn next_for(current: Option<SortState>, key: SortKey) -> SortState {
        match current {
            Some(s) if s.key == key => SortState {
                key,
                ascending: !s.ascending,
            },
            _ => SortState {
                key,
                ascending: true,
            },
        }
    }
}

pub fn sort_people(rows: &mut [Person], state: SortState) {
    rows.sort_by(|a, b| {
        let ord = match state.key {
            SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortKey::Role => a.role.cmp(&b.role).then_with(|| a.name.cmp(&b.name)),
            SortKey::Score => a.score.cmp(&b.score).then_with(|| a.name.cmp(&b.name)),
        };
        if state.ascending {
            ord
        } else {
            ord.reverse()
        }
    });
}

/// Number of pages. Always at least 1, so an empty table still shows "Page 1 of 1".
pub fn page_count(total: usize, page_size: usize) -> usize {
    assert!(page_size > 0, "page_size must be positive");
    total.div_ceil(page_size).max(1)
}

/// Rows on a 0-based page. Out-of-range pages are clamped to the last page.
pub fn page_slice(rows: &[Person], page: usize, page_size: usize) -> &[Person] {
    let last = page_count(rows.len(), page_size) - 1;
    let page = page.min(last);
    let start = page * page_size;
    let end = (start + page_size).min(rows.len());
    &rows[start..end]
}

/// Test data. Names are invented and contain Latin and Persian entries.
pub fn sample_people() -> Vec<Person> {
    let raw = [
        ("Maryam", "Editor", 72),
        ("Arman", "Admin", 91),
        ("Zahra", "Viewer", 48),
        ("Bardia", "Editor", 65),
        ("Chloe", "Admin", 88),
        ("Dariush", "Viewer", 30),
        ("Elham", "Editor", 77),
        ("Farhad", "Viewer", 54),
        ("گلناز", "Admin", 69),
        ("Hossein", "Editor", 41),
    ];
    raw.iter()
        .map(|(n, r, s)| Person {
            name: (*n).to_string(),
            role: (*r).to_string(),
            score: *s,
        })
        .collect()
}

/// Motion preference as the spike understands it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionPref {
    Full,
    Reduced,
}

/// Decision U-10 (2026-10-31): an unknown or unreadable OS setting means reduced motion.
pub fn resolve_motion(os_animations_enabled: Option<bool>) -> MotionPref {
    match os_animations_enabled {
        Some(true) => MotionPref::Full,
        Some(false) | None => MotionPref::Reduced,
    }
}

/// Parses GNOME `enable-animations` output such as `true` or `false`.
pub fn parse_gsettings_bool(output: &str) -> Option<bool> {
    match output.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Reads KDE's `[KDE] AnimationDurationFactor` from a kdeglobals file.
/// A factor of 0 means animations are off. Returns None when the key is missing or invalid.
pub fn parse_kde_animation_factor(contents: &str) -> Option<bool> {
    let mut in_kde_section = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_kde_section = line == "[KDE]";
            continue;
        }
        if in_kde_section {
            if let Some(value) = line.strip_prefix("AnimationDurationFactor=") {
                return match value.trim().parse::<f64>() {
                    Ok(f) if f >= 0.0 => Some(f > 0.0),
                    _ => None,
                };
            }
        }
    }
    None
}

/// Reads the Linux animation setting. GNOME first, then KDE. Returns None if neither is readable.
pub fn read_linux_animations(xdg_config_home: Option<&Path>) -> Option<bool> {
    let gnome = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| parse_gsettings_bool(&String::from_utf8_lossy(&o.stdout)));
    if gnome.is_some() {
        return gnome;
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let kde_path = match xdg_config_home {
        Some(p) => p.join("kdeglobals"),
        None => home?.join(".config").join("kdeglobals"),
    };
    let contents = std::fs::read_to_string(kde_path).ok()?;
    parse_kde_animation_factor(&contents)
}

/// Windows reader. Not implemented in this spike; see spike-results.md (U-10, Windows row).
/// Returns None, so the fallback applies on Windows until the reader is written and tested.
pub fn read_windows_animations() -> Option<bool> {
    None
}

/// Reads the OS setting for the current platform.
pub fn read_os_animations() -> Option<bool> {
    if cfg!(target_os = "windows") {
        read_windows_animations()
    } else {
        let xdg = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from);
        read_linux_animations(xdg.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_by_name_both_directions() {
        let mut rows = sample_people();
        sort_people(
            &mut rows,
            SortState {
                key: SortKey::Name,
                ascending: true,
            },
        );
        assert_eq!(rows.first().unwrap().name, "Arman");
        sort_people(
            &mut rows,
            SortState {
                key: SortKey::Name,
                ascending: false,
            },
        );
        assert_eq!(
            rows.first().unwrap().name,
            "گلناز",
            "Persian sorts after Latin in code point order"
        );
    }

    #[test]
    fn sort_by_score_ascending_is_monotonic() {
        let mut rows = sample_people();
        sort_people(
            &mut rows,
            SortState {
                key: SortKey::Score,
                ascending: true,
            },
        );
        assert!(rows.windows(2).all(|w| w[0].score <= w[1].score));
    }

    #[test]
    fn header_click_toggles_direction() {
        let first = SortState::next_for(None, SortKey::Role);
        assert!(first.ascending);
        let second = SortState::next_for(Some(first), SortKey::Role);
        assert!(!second.ascending);
        let other = SortState::next_for(Some(second), SortKey::Score);
        assert!(other.ascending, "a new column starts ascending");
    }

    #[test]
    fn page_count_edges() {
        assert_eq!(page_count(0, 4), 1);
        assert_eq!(page_count(4, 4), 1);
        assert_eq!(page_count(5, 4), 2);
        assert_eq!(page_count(10, 4), 3);
    }

    #[test]
    fn page_slice_clamps_and_sizes() {
        let rows = sample_people();
        assert_eq!(page_slice(&rows, 0, 4).len(), 4);
        assert_eq!(page_slice(&rows, 2, 4).len(), 2, "last page is partial");
        assert_eq!(
            page_slice(&rows, 99, 4).len(),
            2,
            "out-of-range page clamps to last"
        );
        assert!(page_slice(&[], 0, 4).is_empty());
    }

    #[test]
    fn motion_unknown_falls_back_to_reduced() {
        assert_eq!(resolve_motion(None), MotionPref::Reduced);
        assert_eq!(resolve_motion(Some(false)), MotionPref::Reduced);
        assert_eq!(resolve_motion(Some(true)), MotionPref::Full);
    }

    #[test]
    fn gsettings_output_parsing() {
        assert_eq!(parse_gsettings_bool("true\n"), Some(true));
        assert_eq!(parse_gsettings_bool("false"), Some(false));
        assert_eq!(parse_gsettings_bool("garbage"), None);
        assert_eq!(parse_gsettings_bool(""), None);
    }

    #[test]
    fn kde_animation_factor_parsing() {
        let off = "[General]\nfoo=1\n[KDE]\nAnimationDurationFactor=0\n";
        let on = "[KDE]\nAnimationDurationFactor=1\n";
        assert_eq!(parse_kde_animation_factor(off), Some(false));
        assert_eq!(parse_kde_animation_factor(on), Some(true));
        assert_eq!(
            parse_kde_animation_factor("[KDE]\nAnimationDurationFactor=abc\n"),
            None
        );
        assert_eq!(
            parse_kde_animation_factor("[KDE]\nAnimationDurationFactor=-1\n"),
            None
        );
        assert_eq!(
            parse_kde_animation_factor("[Other]\nAnimationDurationFactor=0\n"),
            None
        );
    }
}
