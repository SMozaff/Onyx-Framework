//! Proves `AppState::new` correctly wires the full composition root
//! (both aggregate types' command/query registrations, the event bus,
//! and the sync agent) against a real SQLite database, by dispatching a
//! real `CreateMission` command through it end-to-end.
//!
//! Does not attempt to exercise the sync agent's actual network
//! behavior — `cloud_relay_socket_factory`/`cloud_relay_auth_provider`
//! are test-only doubles (see their doc comments below), since no
//! production `RelaySocketFactory`/`AuthorityProvider` implementation
//! exists anywhere in the workspace yet (flagged, not silently assumed
//! complete — `sync_transport::cloud_relay::RelaySocket`'s own doc
//! comment states it is "Implemented by the composition root (binds to
//! `tokio-tungstenite` + `reqwest` there)", meaning a real implementation
//! is `client-composition`'s own still-outstanding responsibility, not
//! something this test claims to provide).

use client_composition::{AppState, AppStateConfig};
use mission_domain::test_support::test_user_id;
use platform_contracts::CommandEnvelope;
use platform_kernel::{
    AuthorityEpoch, AuthorityProof, AuthorityScope, CommandId, CorrelationId, DomainObjectRef,
    LifecycleEpoch, ObjectId, ObjectVersion, OrganizationId, ProofType, ReplicaId, SchemaVersion,
    Timestamp, VectorClock,
};
use std::sync::Arc;
use sync_transport::cloud_relay::{RelaySocket, RelaySocketFactory};
use sync_transport::placeholder_types::{AuthorityError, AuthorityProvider};
use sync_transport::TransportError;

#[path = "support/mod.rs"]
#[allow(dead_code)]
// Not every item `support` exports is used by every test binary that includes it — each `tests/*.rs` file is a separate compilation unit under Cargo's convention, and this one only needs `test_pool`.
mod support;
use support::test_pool;

/// Test-only double, always returning a fixed token — the same role as
/// `sync_transport::placeholder_types::StaticAuthorityProvider`, which
/// that crate itself documents as test-only and does not export.
struct StubAuthorityProvider;

#[async_trait::async_trait]
impl AuthorityProvider for StubAuthorityProvider {
    async fn bearer_token(&self) -> Result<String, AuthorityError> {
        Ok("stub-token".to_string())
    }
}

/// Test-only double: never actually opens a socket. `AppState::new`
/// only needs a value satisfying the trait to construct
/// `CloudRelayTransport`; nothing in this test exercises an actual
/// connection attempt.
struct StubRelaySocketFactory;

#[async_trait::async_trait]
impl RelaySocketFactory for StubRelaySocketFactory {
    async fn connect(
        &self,
        _relay_url: &str,
        _peer: &sync_transport::PeerInfo,
        _bearer_token: &str,
        _timeout: std::time::Duration,
    ) -> Result<Box<dyn RelaySocket>, TransportError> {
        Err(TransportError::Unreachable)
    }
}

fn test_config() -> AppStateConfig {
    AppStateConfig {
        local_replica: ReplicaId::new_random(),
        organization_id: OrganizationId::new_random(),
        sync_agent_config: client_composition::SyncAgentConfig::default(),
        event_bus_capacity: 16,
        cloud_relay_endpoint: "wss://relay.test.invalid/v1".to_string(),
        cloud_relay_auth_provider: Arc::new(StubAuthorityProvider),
        cloud_relay_socket_factory: Arc::new(StubRelaySocketFactory),
    }
}

fn envelope_for(
    command_type: &str,
    target_id: ObjectId,
    organization_id: OrganizationId,
    payload: serde_json::Value,
) -> CommandEnvelope<serde_json::Value> {
    CommandEnvelope {
        command_id: CommandId::new_random(),
        operation_id: platform_kernel::OperationId::new_random(),
        command_type: command_type.to_string(),
        schema_version: SchemaVersion::new("1.0.0"),
        target: DomainObjectRef {
            id: target_id,
            r#type: "mission".to_string(),
            organization_id,
        },
        expected_version: ObjectVersion(0),
        expected_lifecycle_epoch: LifecycleEpoch(0),
        expected_authority_epoch: AuthorityEpoch(0),
        actor: platform_kernel::ActorContext {
            user_id: test_user_id(),
            device_id: ObjectId::new_random(),
            organization_id,
        },
        authority_proof: AuthorityProof {
            proof_type: ProofType::Jwt,
            scope: AuthorityScope {
                organization_id,
                object_type: "mission".to_string(),
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
async fn app_state_new_wires_a_working_command_registry_end_to_end() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();

    let state = AppState::new(pool, test_config());

    let envelope = envelope_for(
        "CreateMission",
        ObjectId::new_random(),
        organization_id,
        serde_json::json!({
            "CreateMission": {
                "name": "AppState Wiring Test Mission",
                "description": null,
                "owner_id": test_user_id(),
            }
        }),
    );

    let result =
        state.command_registry.dispatch(envelope).await.expect(
            "CreateMission dispatched through AppState's real CommandRegistry should succeed",
        );
    assert_eq!(result["success"], serde_json::json!(true));

    // Confirm the query side is also correctly wired: GetMission should
    // find the just-created mission.
    let mission_id: ObjectId = serde_json::from_value(result["mission_id"].clone()).unwrap();
    let query_response = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "GetMission".to_string(),
            target_id: mission_id,
        })
        .await
        .expect("GetMission dispatched through AppState's real QueryRegistry should succeed");
    assert_ne!(
        query_response,
        serde_json::Value::Null,
        "the created mission should be findable via GetMission"
    );

    // Confirm the event bus is genuinely constructed and subscribable
    // (not exercising the outbox pump itself here — that's
    // sync_agent.rs's own test coverage).
    let _subscription = state
        .event_bus
        .subscribe(client_composition::event_bus::EventFilter {
            organization_id,
            event_types: None,
        });

    // Confirm the sync agent's status is queryable (proves it and its
    // TransportSelector/CompositeDiscovery/OutboxStore all constructed
    // without panicking).
    let status = state.sync_agent.status().await;
    assert!(
        !status.online,
        "a freshly constructed agent that has not yet run a sync cycle should report offline"
    );
}

#[tokio::test]
async fn app_state_new_wires_task_commands_independently_of_mission_commands() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();

    let state = AppState::new(pool, test_config());

    let mission_id = ObjectId::new_random();
    let envelope = CommandEnvelope {
        target: DomainObjectRef {
            id: ObjectId::new_random(),
            r#type: "task".to_string(),
            organization_id,
        },
        ..envelope_for(
            "CreateTask",
            ObjectId::new_random(),
            organization_id,
            serde_json::json!({
                "CreateTask": {
                    "mission_id": mission_id,
                    "title": "AppState Task Wiring Test",
                    "description": null,
                    "owner_id": test_user_id(),
                }
            }),
        )
    };

    let result = state.command_registry.dispatch(envelope).await.expect(
        "CreateTask dispatched through the same AppState's CommandRegistry should also succeed",
    );
    assert_eq!(result["success"], serde_json::json!(true));
    assert!(result.get("task_id").is_some());
}
