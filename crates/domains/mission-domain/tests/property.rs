//! Property-based tests for the Mission aggregate. Marked `#[ignore]` per
//! §10 of the frozen contract (`cargo test --test property -- --ignored`),
//! since property tests are slower than unit tests and are run
//! deliberately, not on every default `cargo test`.

use mission_domain::test_support::test_context;
use mission_domain::{Mission, MissionCommand, MissionError, MissionStatus};
use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;
use proptest::prelude::*;

fn new_draft_mission() -> Mission {
    let ctx = test_context();
    let events = Mission::create(
        MissionCommand::CreateMission {
            name: "Property Mission".to_string(),
            description: None,
            owner_id: mission_domain::test_support::test_user_id(),
        },
        &ctx,
    )
    .unwrap();
    Mission::from_created_event(&events[0])
}

/// A small, closed alphabet of commands that are always *syntactically*
/// well-formed (non-empty strings, valid evidence lists) so that any
/// rejection observed is a genuine state-gating rejection, not incidental
/// field validation noise.
fn arbitrary_command() -> impl Strategy<Value = MissionCommand> {
    prop_oneof![
        Just(MissionCommand::CreateBlueprintRevision {
            blueprint_data: "data".to_string(),
            version: "1.0".to_string(),
        }),
        Just(MissionCommand::SubmitBlueprint {
            revision_id: ObjectId::new_random(),
        }),
        Just(MissionCommand::RequestApproval {
            reason: "r".to_string(),
            evidence: vec![],
        }),
        Just(MissionCommand::RejectApproval {
            reason: "r".to_string(),
        }),
        Just(MissionCommand::ActivateMission {
            reason: "r".to_string(),
        }),
        Just(MissionCommand::PauseMission {
            reason: "r".to_string(),
        }),
        Just(MissionCommand::OperationalHaltMission {
            reason: "r".to_string(),
            emergency_code: "E-1".to_string(),
        }),
        Just(MissionCommand::ResumeMission {
            reason: "r".to_string(),
        }),
        Just(MissionCommand::RestartMission {
            reason: "r".to_string(),
            checklist_completed: true,
        }),
        Just(MissionCommand::RestartMission {
            reason: "r".to_string(),
            checklist_completed: false,
        }),
        Just(MissionCommand::TriggerReview {
            reason: "r".to_string(),
        }),
        Just(MissionCommand::CloseMission {
            reason: "r".to_string(),
            closure_evidence: vec![ObjectId::new_random()],
        }),
        Just(MissionCommand::ArchiveMission {
            reason: "r".to_string(),
        }),
        Just(MissionCommand::CancelMission {
            reason: "r".to_string(),
        }),
    ]
}

proptest! {
    /// INVARIANT: `decide()` never panics for any command, from any
    /// reachable status, and only ever returns `Ok` or a well-formed
    /// `MissionError` — never a Rust panic/abort. This is the aggregate's
    /// most fundamental safety property: it must be *total* over its
    /// command input, even for nonsensical sequences.
    #[test]
    #[ignore]
    fn decide_never_panics_for_any_command_sequence(
        commands in proptest::collection::vec(arbitrary_command(), 0..20)
    ) {
        let mut mission = new_draft_mission();
        let ctx = test_context();
        for command in commands {
            match mission.decide(command, &ctx) {
                Ok(events) => {
                    for event in &events {
                        mission.apply(event);
                    }
                }
                Err(_typed_domain_error) => {
                    // Rejected: reaching this arm at all already proves no
                    // panic occurred, and the error is a genuine typed
                    // `MissionError` by construction of `decide()`'s
                    // return type.
                }
            }
        }
    }

    /// INVARIANT: version increments by exactly one per successfully
    /// applied event batch that contains exactly one event (true for every
    /// Mission command in Increment 1 -- none produce zero or multiple
    /// events).
    #[test]
    #[ignore]
    fn version_increments_monotonically_with_successful_commands(
        commands in proptest::collection::vec(arbitrary_command(), 0..20)
    ) {
        let mut mission = new_draft_mission();
        let ctx = test_context();
        let mut expected_version = mission.version().0;
        for command in commands {
            if let Ok(events) = mission.decide(command, &ctx) {
                for event in &events {
                    mission.apply(event);
                    expected_version += 1;
                }
            }
        }
        prop_assert_eq!(mission.version().0, expected_version);
    }

    /// INVARIANT: a rejected command never mutates the aggregate. This is
    /// enforced structurally (decide() takes &self), but this test locks
    /// the *observable* consequence in place: status/version/name are
    /// unchanged after any `Err` result.
    #[test]
    #[ignore]
    fn rejected_command_leaves_aggregate_unchanged(
        commands in proptest::collection::vec(arbitrary_command(), 1..20)
    ) {
        let mut mission = new_draft_mission();
        let ctx = test_context();
        for command in commands {
            let status_before = mission.status();
            let version_before = mission.version();
            let name_before = mission.name().to_string();

            let result = mission.decide(command, &ctx);
            match result {
                Ok(events) => {
                    for event in &events {
                        mission.apply(event);
                    }
                }
                Err(_) => {
                    prop_assert_eq!(mission.status(), status_before);
                    prop_assert_eq!(mission.version(), version_before);
                    prop_assert_eq!(mission.name(), name_before.as_str());
                }
            }
        }
    }

    /// INVARIANT: `Archived` is terminal in Increment 1 -- once reached, no
    /// command sequence can move the mission out of `Archived` (ruling A).
    #[test]
    #[ignore]
    fn archived_is_terminal_under_any_further_commands(
        commands in proptest::collection::vec(arbitrary_command(), 0..10)
    ) {
        let mut mission = new_draft_mission();
        let ctx = test_context();

        // Deterministically drive to Archived via Draft -> Cancelled ->
        // Archived (the shortest ruled path).
        let events = mission
            .decide(MissionCommand::CancelMission { reason: "r".to_string() }, &ctx)
            .unwrap();
        for e in &events { mission.apply(e); }
        let events = mission
            .decide(MissionCommand::ArchiveMission { reason: "r".to_string() }, &ctx)
            .unwrap();
        for e in &events { mission.apply(e); }
        prop_assert_eq!(mission.status(), MissionStatus::Archived);

        for command in commands {
            let result = mission.decide(command, &ctx);
            prop_assert!(matches!(result, Err(MissionError::InvalidTransition(_))));
            prop_assert_eq!(mission.status(), MissionStatus::Archived);
        }
    }
}
