//! The StaffProfile aggregate root.
//!
//! Requested 2026-08-13: "a full detailed profile page for each Staff
//! member... shows info publicly and access for modifications for
//! admin." Confirmed shape:
//! - Basic identity + organizational info: public to everyone in the
//!   organization.
//! - Work stats: visible only to Top-level Manager and Admin.
//! - Every field is admin-editable only, via a separate admin-only
//!   editing screen (not inline on the public view).
//!
//! # Authority (established precedent)
//! `decide()` checks only the generic
//! `context.authority.is_authorized(...)` stub, matching every other
//! aggregate in this workspace. The confirmed "Admin only, no
//! exceptions" rule is real-per-role enforcement, which — per this
//! workspace's own precedent (see `policy_domain::aggregate`'s header
//! comment) — is the composing application's responsibility, not this
//! pure-domain crate's.
//!
//! # Read visibility is not this crate's concern
//! The confirmed visibility rules (public identity/org info,
//! Manager-gated work stats) describe **who may read what**, not who
//! may write it. This aggregate stores and mutates all three sections
//! uniformly; gating what a given viewer's query response actually
//! contains is the composing application's query/projection layer's
//! job, matching how `policy_domain`'s own docs describe the same
//! read/write authority split.

use crate::command::ProfileCommand;
use crate::error::ProfileError;
use crate::event::ProfileEvent;
use crate::state_machine::ProfileStatus;
use crate::value::{BasicIdentity, OrganizationalInfo, ProfileOwnerId, StaffProfileId, WorkStats};
use platform_contracts::{AggregateRoot, DecisionContext};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectVersion};
use serde::{Deserialize, Serialize};

/// The StaffProfile aggregate: identity, organizational info, and
/// work-stats sections for one staff member, linked to their account
/// via `owner_id`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaffProfile {
    id: StaffProfileId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: ProfileStatus,
    owner_id: ProfileOwnerId,
    identity: BasicIdentity,
    organizational_info: OrganizationalInfo,
    /// Defaults to `WorkStats::unavailable()` at creation — see that
    /// type's doc comment on why a real data source does not exist yet.
    work_stats: WorkStats,
}

impl StaffProfile {
    /// Constructs a new `StaffProfile` from a `CreateProfile` command.
    ///
    /// # Errors
    /// Returns [`ProfileError::InvalidTransition`] if called with any
    /// command other than `CreateProfile`.
    pub fn create(
        command: ProfileCommand,
        context: &DecisionContext,
    ) -> Result<Vec<ProfileEvent>, ProfileError> {
        let ProfileCommand::CreateProfile {
            owner_id,
            identity,
            organizational_info,
        } = command
        else {
            return Err(ProfileError::InvalidTransition(
                "StaffProfile::create() called with a non-CreateProfile command".to_string(),
            ));
        };

        Ok(vec![ProfileEvent::ProfileCreated {
            profile_id: StaffProfileId(context.generated_id_generator.generate_object_id()),
            owner_id,
            identity,
            organizational_info,
            created_by: context.actor.user_id,
            created_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `StaffProfile` by folding its first event
    /// (`ProfileCreated`).
    ///
    /// # Panics
    /// Panics if `event` is not a `ProfileCreated` event.
    pub fn from_created_event(event: &ProfileEvent) -> Self {
        let ProfileEvent::ProfileCreated {
            profile_id,
            owner_id,
            identity,
            organizational_info,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-ProfileCreated event");
        };

        Self {
            id: *profile_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: ProfileStatus::Active,
            owner_id: *owner_id,
            identity: identity.clone(),
            organizational_info: organizational_info.clone(),
            work_stats: WorkStats::unavailable(),
        }
    }

    /// The profile's current lifecycle status.
    pub fn status(&self) -> ProfileStatus {
        self.status
    }

    /// Which user this profile belongs to.
    pub fn owner_id(&self) -> ProfileOwnerId {
        self.owner_id
    }

    /// The profile's current basic identity fields.
    pub fn identity(&self) -> &BasicIdentity {
        &self.identity
    }

    /// The profile's current organizational info fields.
    pub fn organizational_info(&self) -> &OrganizationalInfo {
        &self.organizational_info
    }

    /// The profile's current work-stats fields.
    pub fn work_stats(&self) -> &WorkStats {
        &self.work_stats
    }

    fn invalid(&self, command: &str) -> ProfileError {
        ProfileError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for StaffProfile {
    type Id = StaffProfileId;
    type Command = ProfileCommand;
    type Event = ProfileEvent;
    type Error = ProfileError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        if !context.authority.is_authorized("profile.command") {
            return Err(ProfileError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            ProfileCommand::CreateProfile { .. } => Err(ProfileError::InvalidTransition(
                "CreateProfile must be dispatched via StaffProfile::create(), not decide()"
                    .to_string(),
            )),

            ProfileCommand::UpdateBasicIdentity { identity } => match self.status {
                ProfileStatus::Active => Ok(vec![ProfileEvent::BasicIdentityUpdated {
                    identity,
                    updated_by: context.actor.user_id,
                    updated_at: context.trusted_now,
                }]),
                ProfileStatus::Archived => Err(self.invalid("UpdateBasicIdentity")),
            },

            ProfileCommand::UpdateOrganizationalInfo {
                organizational_info,
            } => match self.status {
                ProfileStatus::Active => Ok(vec![ProfileEvent::OrganizationalInfoUpdated {
                    organizational_info,
                    updated_by: context.actor.user_id,
                    updated_at: context.trusted_now,
                }]),
                ProfileStatus::Archived => Err(self.invalid("UpdateOrganizationalInfo")),
            },

            ProfileCommand::UpdateWorkStats { work_stats } => match self.status {
                ProfileStatus::Active => Ok(vec![ProfileEvent::WorkStatsUpdated {
                    work_stats,
                    updated_by: context.actor.user_id,
                    updated_at: context.trusted_now,
                }]),
                ProfileStatus::Archived => Err(self.invalid("UpdateWorkStats")),
            },

            ProfileCommand::ArchiveProfile => match self.status {
                ProfileStatus::Active => Ok(vec![ProfileEvent::ProfileArchived {
                    archived_by: context.actor.user_id,
                    archived_at: context.trusted_now,
                }]),
                ProfileStatus::Archived => Err(self.invalid("ArchiveProfile")),
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            ProfileEvent::ProfileCreated { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            ProfileEvent::BasicIdentityUpdated { identity, .. } => {
                self.identity = identity.clone();
            }
            ProfileEvent::OrganizationalInfoUpdated {
                organizational_info,
                ..
            } => {
                self.organizational_info = organizational_info.clone();
            }
            ProfileEvent::WorkStatsUpdated { work_stats, .. } => {
                self.work_stats = work_stats.clone();
            }
            ProfileEvent::ProfileArchived { .. } => {
                self.status = ProfileStatus::Archived;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{active_profile, test_context};

    fn apply_all(profile: &mut StaffProfile, events: &[ProfileEvent]) {
        for e in events {
            profile.apply(e);
        }
    }

    #[test]
    fn create_profile_succeeds_with_unavailable_work_stats() {
        let profile = active_profile();
        assert_eq!(profile.status(), ProfileStatus::Active);
        assert_eq!(profile.work_stats(), &WorkStats::unavailable());
    }

    #[test]
    fn create_called_via_decide_is_rejected() {
        let profile = active_profile();
        let ctx = test_context();
        let result = profile.decide(
            ProfileCommand::CreateProfile {
                owner_id: ctx.actor.user_id,
                identity: BasicIdentity::new("X", None, None, None, None).unwrap(),
                organizational_info: OrganizationalInfo {
                    department: None,
                    class_label: None,
                    parent_display_name: None,
                    team_memberships: vec![],
                },
            },
            &ctx,
        );
        assert!(matches!(result, Err(ProfileError::InvalidTransition(_))));
    }

    #[test]
    fn update_basic_identity_succeeds() {
        let mut profile = active_profile();
        let ctx = test_context();
        let new_identity =
            BasicIdentity::new("New Name", None, Some("Engineer".to_string()), None, None).unwrap();

        let events = profile
            .decide(
                ProfileCommand::UpdateBasicIdentity {
                    identity: new_identity.clone(),
                },
                &ctx,
            )
            .expect("update must succeed");
        apply_all(&mut profile, &events);

        assert_eq!(profile.identity(), &new_identity);
    }

    #[test]
    fn update_organizational_info_succeeds() {
        let mut profile = active_profile();
        let ctx = test_context();
        let new_org_info = OrganizationalInfo {
            department: Some("Engineering".to_string()),
            class_label: Some("team_leader".to_string()),
            parent_display_name: Some("Jane Manager".to_string()),
            team_memberships: vec!["Platform Team".to_string()],
        };

        let events = profile
            .decide(
                ProfileCommand::UpdateOrganizationalInfo {
                    organizational_info: new_org_info.clone(),
                },
                &ctx,
            )
            .expect("update must succeed");
        apply_all(&mut profile, &events);

        assert_eq!(profile.organizational_info(), &new_org_info);
    }

    #[test]
    fn update_work_stats_succeeds() {
        let mut profile = active_profile();
        let ctx = test_context();
        let stats = WorkStats {
            assigned_items_count: Some(5),
            completed_items_count: Some(3),
            target_summary: Some("On track".to_string()),
        };

        let events = profile
            .decide(
                ProfileCommand::UpdateWorkStats {
                    work_stats: stats.clone(),
                },
                &ctx,
            )
            .expect("update must succeed");
        apply_all(&mut profile, &events);

        assert_eq!(profile.work_stats(), &stats);
    }

    #[test]
    fn archive_active_profile_succeeds() {
        let mut profile = active_profile();
        let ctx = test_context();
        let events = profile
            .decide(ProfileCommand::ArchiveProfile, &ctx)
            .expect("archive must succeed");
        apply_all(&mut profile, &events);
        assert_eq!(profile.status(), ProfileStatus::Archived);
    }

    #[test]
    fn archive_twice_is_rejected() {
        let mut profile = active_profile();
        let ctx = test_context();
        let events = profile
            .decide(ProfileCommand::ArchiveProfile, &ctx)
            .unwrap();
        apply_all(&mut profile, &events);
        let result = profile.decide(ProfileCommand::ArchiveProfile, &ctx);
        assert!(matches!(result, Err(ProfileError::InvalidTransition(_))));
    }

    #[test]
    fn update_on_archived_profile_is_rejected() {
        let mut profile = active_profile();
        let ctx = test_context();
        let archive_events = profile
            .decide(ProfileCommand::ArchiveProfile, &ctx)
            .unwrap();
        apply_all(&mut profile, &archive_events);

        let result = profile.decide(
            ProfileCommand::UpdateBasicIdentity {
                identity: BasicIdentity::new("X", None, None, None, None).unwrap(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(ProfileError::InvalidTransition(_))));
    }

    #[test]
    fn version_advances_by_exactly_one_per_applied_event() {
        let profile = active_profile();
        let ctx = test_context();
        let start = profile.version();

        let mut updated = profile.clone();
        for e in profile
            .decide(
                ProfileCommand::UpdateBasicIdentity {
                    identity: BasicIdentity::new("Y", None, None, None, None).unwrap(),
                },
                &ctx,
            )
            .unwrap()
        {
            updated.apply(&e);
        }
        assert_eq!(updated.version(), start.next());
    }
}
