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
        local_discovery: None,
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

/// Proves the Communication wiring end to end through the real composition
/// root: CreateConversation, AddMember, PostMessage, and EditMessage all
/// dispatch through `AppState`'s `CommandRegistry` against real SQLite, and
/// both `GetConversation`/`GetMessage` find what was written — the same
/// path `desktop-shell`'s `execute_command`/`execute_query` Tauri commands
/// and `mobile-core`'s FFI equivalents actually call. Compiling was never
/// the bar; this is the evidence the wiring in `app_state.rs` is correct.
#[tokio::test]
async fn app_state_new_wires_conversation_and_message_commands_end_to_end() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let state = AppState::new(pool, test_config());

    // 1. Create a conversation.
    let create_conversation = envelope_for(
        "CreateConversation",
        ObjectId::new_random(),
        organization_id,
        serde_json::json!({ "CreateConversation": { "conversation_type": "Channel" } }),
    );
    let create_conversation = CommandEnvelope {
        target: DomainObjectRef {
            id: ObjectId::new_random(),
            r#type: "conversation".to_string(),
            organization_id,
        },
        ..create_conversation
    };
    let created = state
        .command_registry
        .dispatch(create_conversation)
        .await
        .expect("CreateConversation should succeed through the real CommandRegistry");
    assert_eq!(created["success"], serde_json::json!(true));
    let conversation_id: ObjectId =
        serde_json::from_value(created["conversation_id"].clone()).unwrap();

    // 2. Add a member — a decision command, proving the Conversation
    // DecisionHandler path (load -> decide -> commit) also works.
    let new_member = test_user_id();
    let add_member = CommandEnvelope {
        target: DomainObjectRef {
            id: conversation_id,
            r#type: "conversation".to_string(),
            organization_id,
        },
        ..envelope_for(
            "AddMember",
            conversation_id,
            organization_id,
            serde_json::json!({ "AddMember": { "user_id": new_member } }),
        )
    };
    let add_result = state
        .command_registry
        .dispatch(add_member)
        .await
        .expect("AddMember should succeed against the freshly created conversation");
    assert_eq!(add_result["success"], serde_json::json!(true));
    assert_eq!(add_result["new_version"], serde_json::json!(1));

    // 3. GetConversation should reflect the added member.
    let conversation_query = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "GetConversation".to_string(),
            target_id: conversation_id,
        })
        .await
        .expect("GetConversation should find the conversation");
    let members = conversation_query["aggregate"]["members"]
        .as_array()
        .expect("members must be a JSON array");
    assert_eq!(members.len(), 2, "creator plus the newly added member");

    // 4. Post a message into that conversation.
    let post_message = CommandEnvelope {
        target: DomainObjectRef {
            id: ObjectId::new_random(),
            r#type: "message".to_string(),
            organization_id,
        },
        ..envelope_for(
            "PostMessage",
            ObjectId::new_random(),
            organization_id,
            serde_json::json!({
                "PostMessage": {
                    "conversation_id": conversation_id,
                    "body": "hello from the wiring test",
                }
            }),
        )
    };
    let posted = state
        .command_registry
        .dispatch(post_message)
        .await
        .expect("PostMessage should succeed through the real CommandRegistry");
    assert_eq!(posted["success"], serde_json::json!(true));
    let message_id: ObjectId = serde_json::from_value(posted["message_id"].clone()).unwrap();

    // 5. Edit it — proving the Message DecisionHandler path too.
    let edit_message = CommandEnvelope {
        target: DomainObjectRef {
            id: message_id,
            r#type: "message".to_string(),
            organization_id,
        },
        ..envelope_for(
            "EditMessage",
            message_id,
            organization_id,
            serde_json::json!({ "EditMessage": { "new_body": "edited via wiring test" } }),
        )
    };
    let edited = state
        .command_registry
        .dispatch(edit_message)
        .await
        .expect("EditMessage should succeed against the freshly posted message");
    assert_eq!(edited["success"], serde_json::json!(true));

    // 6. GetMessage should reflect the edit, proving persistence round-trips
    // through the real SQLite-backed Repository, not just in-memory state.
    let message_query = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "GetMessage".to_string(),
            target_id: message_id,
        })
        .await
        .expect("GetMessage should find the message");
    assert_eq!(
        message_query["aggregate"]["body"],
        serde_json::json!("edited via wiring test")
    );
    assert_eq!(
        message_query["aggregate"]["status"],
        serde_json::json!("Edited")
    );
}

/// Proves the File wiring end to end through the real composition root:
/// CreateFileAsset, GrantFileAccess, StartUpload, AppendChunk, and
/// FinalizeUpload all dispatch through `AppState`'s `CommandRegistry`
/// against real SQLite, and both `GetFileAsset`/`GetUploadSession` find
/// what was written — the same path `desktop-shell`'s
/// `execute_command`/`execute_query` Tauri commands and `mobile-core`'s
/// FFI equivalents actually call. Compiling was never the bar; this is
/// the evidence the wiring in `app_state.rs` is correct, matching the
/// rigor already proven for Communication above.
#[tokio::test]
async fn app_state_new_wires_file_asset_and_upload_session_commands_end_to_end() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let state = AppState::new(pool, test_config());

    // 1. Create a file asset.
    let file_asset_target = ObjectId::new_random();
    let create_file_asset = CommandEnvelope {
        target: DomainObjectRef {
            id: file_asset_target,
            r#type: "file_asset".to_string(),
            organization_id,
        },
        ..envelope_for(
            "CreateFileAsset",
            file_asset_target,
            organization_id,
            serde_json::json!({
                "CreateFileAsset": {
                    "file_name": "sprint-plan.pdf",
                    "mime_type": "application/pdf",
                }
            }),
        )
    };
    let created = state
        .command_registry
        .dispatch(create_file_asset)
        .await
        .expect("CreateFileAsset should succeed through the real CommandRegistry");
    assert_eq!(created["success"], serde_json::json!(true));
    let file_asset_id: ObjectId = serde_json::from_value(created["file_asset_id"].clone()).unwrap();

    // 2. Grant access to another user — a decision command, proving the
    // FileAsset DecisionHandler path (load -> decide -> commit) also works.
    let grantee = test_user_id();
    let grant_access = CommandEnvelope {
        target: DomainObjectRef {
            id: file_asset_id,
            r#type: "file_asset".to_string(),
            organization_id,
        },
        ..envelope_for(
            "GrantFileAccess",
            file_asset_id,
            organization_id,
            serde_json::json!({ "GrantFileAccess": { "user_id": grantee } }),
        )
    };
    let grant_result = state
        .command_registry
        .dispatch(grant_access)
        .await
        .expect("GrantFileAccess should succeed against the freshly created file asset");
    assert_eq!(grant_result["success"], serde_json::json!(true));
    assert_eq!(grant_result["new_version"], serde_json::json!(1));

    // 3. GetFileAsset should reflect the granted access.
    let file_asset_query = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "GetFileAsset".to_string(),
            target_id: file_asset_id,
        })
        .await
        .expect("GetFileAsset should find the file asset");
    let access = file_asset_query["aggregate"]["access"]
        .as_array()
        .expect("access must be a JSON array");
    assert_eq!(access.len(), 2, "owner plus the newly granted user");

    // 4. Start an upload session targeting that file asset, well within
    // the 100 MB cap.
    let upload_target = ObjectId::new_random();
    let start_upload = CommandEnvelope {
        target: DomainObjectRef {
            id: upload_target,
            r#type: "upload_session".to_string(),
            organization_id,
        },
        ..envelope_for(
            "StartUpload",
            upload_target,
            organization_id,
            serde_json::json!({
                "StartUpload": {
                    "file_asset_id": file_asset_id,
                    "total_size": 8,
                }
            }),
        )
    };
    let started = state
        .command_registry
        .dispatch(start_upload)
        .await
        .expect("StartUpload should succeed through the real CommandRegistry");
    assert_eq!(started["success"], serde_json::json!(true));
    let upload_session_id: ObjectId =
        serde_json::from_value(started["upload_session_id"].clone()).unwrap();

    // 5. Append the single chunk covering the declared total.
    let append_chunk = CommandEnvelope {
        target: DomainObjectRef {
            id: upload_session_id,
            r#type: "upload_session".to_string(),
            organization_id,
        },
        ..envelope_for(
            "AppendChunk",
            upload_session_id,
            organization_id,
            serde_json::json!({
                "AppendChunk": {
                    "chunk_index": 0,
                    "chunk_size": 8,
                    "chunk_hash": "chunk-0-hash",
                }
            }),
        )
    };
    let appended = state
        .command_registry
        .dispatch(append_chunk)
        .await
        .expect("AppendChunk should succeed against the freshly started upload session");
    assert_eq!(appended["success"], serde_json::json!(true));

    // 6. Finalize — proving the UploadSession DecisionHandler path fully
    // round-trips through real SQLite, not just in-memory state.
    let finalize_upload = CommandEnvelope {
        target: DomainObjectRef {
            id: upload_session_id,
            r#type: "upload_session".to_string(),
            organization_id,
        },
        ..envelope_for(
            "FinalizeUpload",
            upload_session_id,
            organization_id,
            serde_json::json!({ "FinalizeUpload": { "final_hash": "final-hash" } }),
        )
    };
    let finalized = state
        .command_registry
        .dispatch(finalize_upload)
        .await
        .expect("FinalizeUpload should succeed once all declared bytes were received");
    assert_eq!(finalized["success"], serde_json::json!(true));

    // 7. GetUploadSession should reflect the finalized status.
    let upload_session_query = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "GetUploadSession".to_string(),
            target_id: upload_session_id,
        })
        .await
        .expect("GetUploadSession should find the upload session");
    assert_eq!(
        upload_session_query["aggregate"]["status"],
        serde_json::json!("Finalized")
    );
}
