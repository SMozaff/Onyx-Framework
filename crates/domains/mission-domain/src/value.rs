//! Value types for the Mission bounded context.

use platform_kernel::ObjectId;
use serde::{Deserialize, Serialize};

/// The identity of a Mission aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MissionId(pub ObjectId);

impl MissionId {
    /// Generates a new random mission identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// A reference to a mission's timeline (owned by a separate bounded
/// context introduced in a later increment).
///
/// # Increment 1 Ruling (D)
/// Placeholder newtype. The Team Prompt's file manifest and module exports
/// reference `MissionTimelineRef`, but no increment document defines its
/// shape. Ruled: a thin wrapper over `ObjectId`, matching the
/// `MissionSettings`/`Dependency` placeholder pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionTimelineRef(pub ObjectId);

/// Mission-level configuration settings.
///
/// # Increment 1 Ruling (D)
/// Placeholder struct with `serde` defaults, per ruling.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionSettings {
    /// The mission's operating timezone (IANA name, e.g. `"UTC"`).
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// The calendar system or scheduling calendar identifier in use.
    #[serde(default = "default_calendar")]
    pub calendar: String,
}

fn default_timezone() -> String {
    "UTC".to_string()
}

fn default_calendar() -> String {
    "standard".to_string()
}

impl Default for MissionSettings {
    fn default() -> Self {
        Self {
            timezone: default_timezone(),
            calendar: default_calendar(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_id_random_generation_is_unique() {
        let a = MissionId::new_random();
        let b = MissionId::new_random();
        assert_ne!(a, b);
    }

    #[test]
    fn mission_settings_default_matches_placeholder_ruling() {
        let s = MissionSettings::default();
        assert_eq!(s.timezone, "UTC");
        assert_eq!(s.calendar, "standard");
    }

    #[test]
    fn mission_settings_serde_defaults_apply_on_missing_fields() {
        let json = "{}";
        let s: MissionSettings = serde_json::from_str(json).unwrap();
        assert_eq!(s, MissionSettings::default());
    }
}
