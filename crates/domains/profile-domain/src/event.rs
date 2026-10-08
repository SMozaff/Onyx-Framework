//! Domain events emitted by the StaffProfile aggregate.

use crate::value::{BasicIdentity, OrganizationalInfo, ProfileOwnerId, StaffProfileId, WorkStats};
use platform_kernel::{Timestamp, UserId};
use serde::{Deserialize, Serialize};

/// Events emitted by the StaffProfile aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProfileEvent {
    /// A new profile was created.
    ProfileCreated {
        /// The new profile's identity.
        profile_id: StaffProfileId,
        /// Which user this profile belongs to.
        owner_id: ProfileOwnerId,
        /// The initial basic identity fields.
        identity: BasicIdentity,
        /// The initial organizational info fields.
        organizational_info: OrganizationalInfo,
        /// Who created it (the acting Admin).
        created_by: UserId,
        /// When.
        created_at: Timestamp,
    },
    /// Basic identity was updated.
    BasicIdentityUpdated {
        /// The new identity values.
        identity: BasicIdentity,
        /// Who made the change.
        updated_by: UserId,
        /// When.
        updated_at: Timestamp,
    },
    /// Organizational info was updated.
    OrganizationalInfoUpdated {
        /// The new organizational info values.
        organizational_info: OrganizationalInfo,
        /// Who made the change.
        updated_by: UserId,
        /// When.
        updated_at: Timestamp,
    },
    /// Work stats were updated.
    WorkStatsUpdated {
        /// The new work-stats values.
        work_stats: WorkStats,
        /// Who made the change.
        updated_by: UserId,
        /// When.
        updated_at: Timestamp,
    },
    /// The profile was archived.
    ProfileArchived {
        /// Who archived it.
        archived_by: UserId,
        /// When.
        archived_at: Timestamp,
    },
}
