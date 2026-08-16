//! Deterministic test fixtures for `work-domain`'s test suites, reusing
//! `mission-domain::test_support` for the shared `DecisionContext` and
//! `IdGenerator`.

use mission_domain::MissionId;
use platform_kernel::ObjectId;

pub use mission_domain::test_support::{test_context, test_user_id};

/// Generates a fresh random `MissionId` for use as a task's owning mission
/// in tests.
pub fn test_mission_id() -> MissionId {
    MissionId(ObjectId::new_random())
}
