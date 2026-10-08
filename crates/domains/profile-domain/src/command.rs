//! Commands accepted by the StaffProfile aggregate.

use crate::value::{BasicIdentity, OrganizationalInfo, ProfileOwnerId, WorkStats};
use serde::{Deserialize, Serialize};

/// Commands accepted by the StaffProfile aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProfileCommand {
    /// Create a new profile for a user. Routed through
    /// `StaffProfile::create()`, not `decide()` — same established
    /// pattern as every other aggregate's creation command.
    CreateProfile {
        /// Which user this profile belongs to.
        owner_id: ProfileOwnerId,
        /// The initial basic identity fields.
        identity: BasicIdentity,
        /// The initial organizational info fields.
        organizational_info: OrganizationalInfo,
    },
    /// Replace the profile's basic identity fields. Admin-only per the
    /// confirmed product decision ("only Admin can edit any field") —
    /// enforced by `decide()`'s authority check, same seam every other
    /// aggregate in this workspace uses.
    UpdateBasicIdentity {
        /// The replacement identity values.
        identity: BasicIdentity,
    },
    /// Replace the profile's organizational info fields. Admin-only,
    /// same as above.
    UpdateOrganizationalInfo {
        /// The replacement organizational info values.
        organizational_info: OrganizationalInfo,
    },
    /// Replace the profile's work-stats section. Admin-only. In
    /// practice, until `IMPLEMENTATION_PLAN_User_Hierarchy.md` Phase D
    /// exists, the only legitimate value to pass here is
    /// `WorkStats::unavailable()` — see that type's own doc comment.
    UpdateWorkStats {
        /// The replacement work-stats values.
        work_stats: WorkStats,
    },
    /// Archive the profile (e.g. the owning user was deactivated).
    ArchiveProfile,
}
