//! End-to-end proof that the owner-authority gate added this session
//! actually works, against a real SQLite database and the real
//! `TaskDecisionHandler`/`handle_command` path — not just a unit test
//! of `HierarchyCache::is_authorized_to_decide`'s logic in isolation
//! (that's `desktop-shell::hierarchy`'s own test module).
//!
//! Confirms, with a real dispatch through `CommandRegistry`:
//! - `SubmitCompletion` (the task owner acting on their own work) still
//!   succeeds ungated, run by the owner themself.
//! - `ApproveTask` succeeds when the actor is a real, cache-resolved
//!   direct manager of the task's owner.
//! - `ApproveTask` is denied (`CommandError::OwnerAuthorityDenied`, not
//!   a silent success) when the actor has no authority relationship to
//!   the task's owner at all.

use client_composition::command_registry::CommandRegistry;
use platform_contracts::CommandEnvelope;
use platform_kernel::{
    ActorContext, AuthorityEpoch, AuthorityProof, AuthorityScope, CommandId, CorrelationId,
    DomainObjectRef, LifecycleEpoch, ObjectId, ObjectVersion, OrganizationId, ProofType,
    SchemaVersion, Timestamp, UserId, VectorClock,
};
use std::collections::HashMap;
use std::sync::Arc;

#[path = "support/mod.rs"]
#[allow(dead_code)]
// Not every item `support` exports is used by every test binary that
// includes it — see `app_state_wiring.rs`'s identical annotation.
mod support;
use support::{test_pool, InMemoryIdempotencyStore};

/// Minimal `OwnerAuthority` test double: direct-manager-only, exactly
/// `desktop-shell::hierarchy::HierarchyCache::is_authorized_to_decide`'s
/// contract (an actor is authorized iff they are the owner's mapped
/// manager), reimplemented here rather than pulled in as a dependency —
/// `client-composition` cannot depend on `desktop-shell` (that direction
/// would be backwards: `desktop-shell` depends on `client-composition`).
struct FixedManagerAuthority {
    managers: HashMap<UserId, UserId>,
}

#[async_trait::async_trait]
impl api_server::OwnerAuthority for FixedManagerAuthority {
    async fn is_authorized(&self, actor: UserId, owner: UserId) -> bool {
        self.managers.get(&owner) == Some(&actor)
    }
}

fn envelope_for(
    command_type: &str,
    target_id: ObjectId,
    organization_id: OrganizationId,
    expected_version: u64,
    actor_user_id: UserId,
    payload: serde_json::Value,
) -> CommandEnvelope<serde_json::Value> {
    CommandEnvelope {
        command_id: CommandId::new_random(),
        operation_id: platform_kernel::OperationId::new_random(),
        command_type: command_type.to_string(),
        schema_version: SchemaVersion::new("1.0.0"),
        target: DomainObjectRef {
            id: target_id,
            r#type: "task".to_string(),
            organization_id,
        },
        expected_version: ObjectVersion(expected_version),
        // Every decision command exercised in this test causes a status
        // transition, and `Task::apply` advances `lifecycle_epoch` by
        // one on every such transition (see `aggregate.rs`) — so the
        // lifecycle epoch tracks `expected_version` in lockstep here,
        // both starting at 0 after `CreateTask`.
        expected_lifecycle_epoch: LifecycleEpoch(expected_version),
        expected_authority_epoch: AuthorityEpoch(0),
        actor: ActorContext {
            user_id: actor_user_id,
            device_id: ObjectId::new_random(),
            organization_id,
        },
        authority_proof: AuthorityProof {
            proof_type: ProofType::Jwt,
            scope: AuthorityScope {
                organization_id,
                object_type: "task".to_string(),
                object_id: None,
                command_types: vec![command_type.to_string()],
                delegation_depth: 0,
            },
            issued_at: Timestamp::from_nanos(0),
            expires_at: Timestamp::from_nanos(u64::MAX),
            signature: None,
        },
        issued_at: Timestamp::now(),
        vector_clock: VectorClock::new(),
        correlation_id: CorrelationId::new_random(),
        causation_id: None,
        payload,
    }
}

#[tokio::test]
async fn owner_submits_manager_approves_stranger_denied() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let mission_id = ObjectId::new_random();

    let owner = ObjectId::new_random();
    let manager = ObjectId::new_random();
    let stranger = ObjectId::new_random();

    let authority: Arc<dyn api_server::OwnerAuthority> = Arc::new(FixedManagerAuthority {
        managers: HashMap::from([(owner, manager)]),
    });

    let repo: Arc<dyn query_application::Repository> = Arc::new(
        persistence_sqlite::SqliteRepository::new(pool.clone(), "task"),
    );
    let unit_factory: Arc<dyn query_application::UnitOfWorkFactory> = Arc::new(
        persistence_sqlite::SqliteUnitOfWorkFactory::new(pool.clone()),
    );
    let idempotency_store: Arc<dyn query_application::IdempotencyStore> =
        Arc::new(InMemoryIdempotencyStore::new());

    let mut registry = CommandRegistry::new();
    registry
        .register_creation(
            "CreateTask",
            client_composition::handlers::TaskCreationHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
            ),
        )
        .register_decision(
            "MarkReady",
            client_composition::handlers::TaskDecisionHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
                Arc::clone(&idempotency_store),
                Arc::clone(&authority),
            ),
        )
        .register_decision(
            "StartTask",
            client_composition::handlers::TaskDecisionHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
                Arc::clone(&idempotency_store),
                Arc::clone(&authority),
            ),
        )
        .register_decision(
            "SubmitCompletion",
            client_composition::handlers::TaskDecisionHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
                Arc::clone(&idempotency_store),
                Arc::clone(&authority),
            ),
        )
        .register_decision(
            "ApproveTask",
            client_composition::handlers::TaskDecisionHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
                Arc::clone(&idempotency_store),
                Arc::clone(&authority),
            ),
        );

    // 1. Create the task, owned by `owner`.
    let create_result = registry
        .dispatch(envelope_for(
            "CreateTask",
            ObjectId::new_random(),
            organization_id,
            0,
            owner,
            serde_json::json!({
                "CreateTask": {
                    "mission_id": mission_id,
                    "title": "Owner-authority gate test task",
                    "description": null,
                    "owner_id": owner,
                }
            }),
        ))
        .await
        .expect("CreateTask should succeed");
    let task_id: ObjectId =
        serde_json::from_value(create_result["task_id"].clone()).expect("task_id");

    // 2. Draft -> Ready -> Active -> Submitted, all as the owner (none of
    // these are owner-gated).
    registry
        .dispatch(envelope_for(
            "MarkReady",
            task_id,
            organization_id,
            0,
            owner,
            serde_json::json!({"MarkReady": {"reason": "ready"}}),
        ))
        .await
        .expect("MarkReady should succeed");
    registry
        .dispatch(envelope_for(
            "StartTask",
            task_id,
            organization_id,
            1,
            owner,
            serde_json::json!({"StartTask": {"reason": "starting"}}),
        ))
        .await
        .expect("StartTask should succeed");
    registry
        .dispatch(envelope_for(
            "SubmitCompletion",
            task_id,
            organization_id,
            2,
            owner,
            serde_json::json!({"SubmitCompletion": {"evidence": [ObjectId::new_random()]}}),
        ))
        .await
        .expect(
            "SubmitCompletion must still succeed ungated for the task's own owner \
             (it is the owner acting on their own work, not a decision about someone \
             else's — deliberately excluded from the owner-authority gate)",
        );

    // 3. A stranger with no authority relationship to `owner` tries to
    // approve — must be denied, not silently succeed.
    let denied = registry
        .dispatch(envelope_for(
            "ApproveTask",
            task_id,
            organization_id,
            3,
            stranger,
            serde_json::json!({"ApproveTask": {"reason": "approving"}}),
        ))
        .await;
    assert!(
        denied.is_err(),
        "an unrelated, non-manager user must be denied ApproveTask, not silently succeed \
         — this is the exact gap this session's fix closes"
    );
    let error_message = denied.unwrap_err().to_string();
    assert!(
        error_message.contains("not authorized to decide on behalf of"),
        "the rejection must be OwnerAuthorityDenied specifically, not some other \
         unrelated failure; got: {error_message}"
    );

    // 4. The task's real, cache-resolved direct manager approves —
    // must succeed.
    let approved = registry
        .dispatch(envelope_for(
            "ApproveTask",
            task_id,
            organization_id,
            3,
            manager,
            serde_json::json!({"ApproveTask": {"reason": "approving"}}),
        ))
        .await
        .expect("the task owner's real direct manager must be authorized to approve");
    assert_eq!(approved["success"], serde_json::json!(true));

    let reloaded = repo.load(&task_id).await.unwrap().unwrap();
    assert_eq!(
        reloaded.aggregate["status"],
        serde_json::json!("Approved"),
        "status must have genuinely advanced to Approved after the manager's approval"
    );
}
