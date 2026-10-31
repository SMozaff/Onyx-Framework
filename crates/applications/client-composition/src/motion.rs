//! Reduced-motion policy (U-10).
//!
//! Recommended decision, pending owner confirmation (2026-10-31):
//! - Source: the operating system's reduce-motion setting.
//! - Fallback: when the OS does not report a value, motion is reduced
//!   (fail-safe for users who may be sensitive to motion).
//!
//! Per-OS readers (Windows, macOS, Linux) are not implemented here. Each
//! shell passes the value it reads into [`MotionPreference::resolve`].

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionPreference {
    Full,
    Reduced,
}

impl MotionPreference {
    /// `os_reduce_motion` is `Some(true)` when the OS asks for reduced motion,
    /// `Some(false)` when it allows full motion, and `None` when unknown.
    pub fn resolve(os_reduce_motion: Option<bool>) -> Self {
        match os_reduce_motion {
            Some(false) => Self::Full,
            Some(true) | None => Self::Reduced,
        }
    }

    /// Duration for a transition. Reduced motion removes the animation.
    pub fn duration_ms(self, base_ms: u32) -> u32 {
        match self {
            Self::Full => base_ms,
            Self::Reduced => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_allows_motion_means_full() {
        assert_eq!(
            MotionPreference::resolve(Some(false)),
            MotionPreference::Full
        );
    }

    #[test]
    fn os_requests_reduced_motion_means_reduced() {
        assert_eq!(
            MotionPreference::resolve(Some(true)),
            MotionPreference::Reduced
        );
    }

    #[test]
    fn unknown_setting_falls_back_to_reduced() {
        assert_eq!(MotionPreference::resolve(None), MotionPreference::Reduced);
    }

    #[test]
    fn reduced_motion_removes_transition_duration() {
        assert_eq!(MotionPreference::Full.duration_ms(250), 250);
        assert_eq!(MotionPreference::Reduced.duration_ms(250), 0);
    }
}
