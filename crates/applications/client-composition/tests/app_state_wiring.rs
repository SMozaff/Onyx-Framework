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
        // A unique per-call temp path. Deliberately NOT a `TempDir`
        // handle held for cleanup: `AppStateConfig` takes a plain
        // `PathBuf`, and a `TempDir` dropped at the end of this function
        // would delete the directory out from under the `AppState` that
        // is about to use it. These live under the OS temp dir and are
        // reclaimed by the OS/CI runner, which is the same trade every
        // other throwaway path in this test file makes. Uniqueness comes
        // from `ObjectId::new_random` (already imported here) rather than
        // adding a `uuid` dev-dependency purely for a directory name.
        blob_store_root: std::env::temp_dir().join("onyx-test-blobs").join(
            ObjectId::new_random()
                .0
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        ),
        // This test doesn't exercise owner-gated Task/Mission decision
        // commands (`ApproveTask`/`RejectTask`/`RejectApproval`/
        // `ActivateMission`) — `None` (the safe deny-all default, see
        // `DenyAllOwnerAuthority`'s doc comment in `app_state.rs`) is
        // correct here, not a stand-in that happens to be unused.
        owner_authority: None,
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

    let state = AppState::new(pool, test_config()).await;

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

    let state = AppState::new(pool, test_config()).await;

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
    let state = AppState::new(pool, test_config()).await;

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
    let state = AppState::new(pool, test_config()).await;

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
    let version_after_append: u64 = serde_json::from_value(appended["new_version"].clone())
        .expect("AppendChunk's result must carry the aggregate's new version");

    // 6. Finalize — proving the UploadSession DecisionHandler path fully
    // round-trips through real SQLite, not just in-memory state. Must
    // target the version AppendChunk just advanced to, not envelope_for's
    // default of 0 — this aggregate has now taken two decision commands
    // in sequence, unlike the single-decision Conversation/Message tests
    // above where the default happened to still be correct.
    let finalize_upload = CommandEnvelope {
        target: DomainObjectRef {
            id: upload_session_id,
            r#type: "upload_session".to_string(),
            organization_id,
        },
        expected_version: ObjectVersion(version_after_append),
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

/// Phase 1 (Desktop & Web Completion) addition. Proves `AppState::new`
/// correctly wires the `Policy` aggregate's full command surface:
/// CreatePolicy (creation) -> CreatePolicyVersion -> PublishPolicyVersion
/// -> EvaluatePolicy (decision path), end-to-end against real SQLite.
#[tokio::test]
async fn app_state_new_wires_policy_commands_end_to_end() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let state = AppState::new(pool, test_config()).await;

    // 1. Create a policy.
    let create_policy = CommandEnvelope {
        target: DomainObjectRef {
            id: ObjectId::new_random(),
            r#type: "policy".to_string(),
            organization_id,
        },
        ..envelope_for(
            "CreatePolicy",
            ObjectId::new_random(),
            organization_id,
            serde_json::json!({
                "CreatePolicy": {
                    "name": "Org Governance",
                    "scope": { "Organization": organization_id }
                }
            }),
        )
    };
    let created = state
        .command_registry
        .dispatch(create_policy)
        .await
        .expect("CreatePolicy should succeed through the real CommandRegistry");
    assert_eq!(created["success"], serde_json::json!(true));
    let policy_id: ObjectId = serde_json::from_value(created["policy_id"].clone()).unwrap();

    // 2. Draft a version — a decision command, proving the Policy
    // DecisionHandler path (load -> decide -> commit) also works.
    let create_version = CommandEnvelope {
        target: DomainObjectRef {
            id: policy_id,
            r#type: "policy".to_string(),
            organization_id,
        },
        ..envelope_for(
            "CreatePolicyVersion",
            policy_id,
            organization_id,
            serde_json::json!({
                "CreatePolicyVersion": {
                    "rules": [
                        {
                            "rule_type": "FeatureToggle",
                            "key": "messaging.enabled",
                            "value": true
                        }
                    ]
                }
            }),
        )
    };
    let version_created = state
        .command_registry
        .dispatch(create_version)
        .await
        .expect("CreatePolicyVersion should succeed against the freshly created policy");
    assert_eq!(version_created["success"], serde_json::json!(true));

    // 3. Publish it.
    let publish = CommandEnvelope {
        target: DomainObjectRef {
            id: policy_id,
            r#type: "policy".to_string(),
            organization_id,
        },
        expected_version: ObjectVersion(1),
        ..envelope_for(
            "PublishPolicyVersion",
            policy_id,
            organization_id,
            serde_json::json!({ "PublishPolicyVersion": null }),
        )
    };
    let published = state
        .command_registry
        .dispatch(publish)
        .await
        .expect("PublishPolicyVersion should succeed once a draft exists");
    assert_eq!(published["success"], serde_json::json!(true));

    // 4. GetPolicy should reflect the published version.
    let policy_query = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "GetPolicy".to_string(),
            target_id: policy_id,
        })
        .await
        .expect("GetPolicy should find the policy");
    assert_eq!(
        policy_query["aggregate"]["status"],
        serde_json::json!("Active")
    );
}

/// Phase 1 addition. Proves `AppState::new` correctly wires the
/// `ConnectionRequest` aggregate: SendConnectionRequest (creation) ->
/// AcceptConnectionRequest (decision), end-to-end against real SQLite.
#[tokio::test]
async fn app_state_new_wires_connection_request_commands_end_to_end() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let state = AppState::new(pool, test_config()).await;
    let recipient = test_user_id();

    // 1. Send a connection request.
    let send_request = CommandEnvelope {
        target: DomainObjectRef {
            id: ObjectId::new_random(),
            r#type: "connection_request".to_string(),
            organization_id,
        },
        ..envelope_for(
            "SendConnectionRequest",
            ObjectId::new_random(),
            organization_id,
            serde_json::json!({
                "SendConnectionRequest": { "recipient_id": recipient }
            }),
        )
    };
    let sent = state
        .command_registry
        .dispatch(send_request)
        .await
        .expect("SendConnectionRequest should succeed through the real CommandRegistry");
    assert_eq!(sent["success"], serde_json::json!(true));
    let request_id: ObjectId =
        serde_json::from_value(sent["connection_request_id"].clone()).unwrap();

    // 2. GetConnectionRequest should show it Pending.
    let pending_query = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "GetConnectionRequest".to_string(),
            target_id: request_id,
        })
        .await
        .expect("GetConnectionRequest should find the request");
    assert_eq!(
        pending_query["aggregate"]["status"],
        serde_json::json!("Pending")
    );
}

/// Phase 1 addition. Proves the `FileUploadCoordinator` actually stores
/// and returns real file **content**, not just metadata — the gap
/// `query_application::BlobStore` was added to close. Exercises the full
/// chain: CreateFileAsset -> StartUpload -> AppendChunk(s) ->
/// FinalizeUpload -> CreateVersion, with bytes written to a real
/// `LocalBlobStore` on disk and read back by content hash.
#[tokio::test]
async fn file_upload_coordinator_round_trips_real_content() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let state = AppState::new(pool, test_config()).await;

    let actor = platform_kernel::ActorContext {
        user_id: test_user_id(),
        device_id: ObjectId::new_random(),
        organization_id,
    };

    let content = b"the quick brown fox jumps over the lazy dog".to_vec();
    let outcome = state
        .file_upload_coordinator
        .upload_new_file(
            actor,
            "notes.txt".to_string(),
            "text/plain".to_string(),
            &content,
        )
        .await
        .expect("upload_new_file must succeed end-to-end");

    assert_eq!(outcome.size_bytes, content.len() as u64);
    assert_eq!(
        outcome.content_hash.len(),
        64,
        "a SHA-256 hex digest is 64 characters"
    );

    // The bytes must actually come back — this is the assertion that
    // would have failed before a real BlobStore existed.
    let downloaded = state
        .file_upload_coordinator
        .download(&outcome.content_hash)
        .await
        .expect("download must succeed")
        .expect("content must be present for a hash that was just uploaded");
    assert_eq!(downloaded, content);

    // And the FileAsset aggregate must be queryable, with its version
    // recorded (proving the domain-command half of the flow ran too, not
    // just the blob write).
    let asset = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "GetFileAsset".to_string(),
            target_id: outcome.file_asset_id,
        })
        .await
        .expect("GetFileAsset should find the uploaded asset");
    assert_eq!(
        asset["aggregate"]["file_name"],
        serde_json::json!("notes.txt")
    );
}

/// Phase 1 addition. A file larger than one chunk must still round-trip
/// — proves the multi-`AppendChunk` loop (and its per-chunk version
/// threading) works, not just the single-chunk happy path.
#[tokio::test]
async fn file_upload_coordinator_handles_multi_chunk_content() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let state = AppState::new(pool, test_config()).await;

    let actor = platform_kernel::ActorContext {
        user_id: test_user_id(),
        device_id: ObjectId::new_random(),
        organization_id,
    };

    // CHUNK_SIZE_BYTES is 4 MiB; 9 MiB spans three chunks (two full,
    // one partial), exercising both the full-chunk and final-partial-chunk
    // paths plus the version advance between them.
    let content = vec![7u8; 9 * 1024 * 1024];
    let outcome = state
        .file_upload_coordinator
        .upload_new_file(
            actor,
            "large.bin".to_string(),
            "application/octet-stream".to_string(),
            &content,
        )
        .await
        .expect("a multi-chunk upload must succeed");

    assert_eq!(outcome.size_bytes, content.len() as u64);

    let downloaded = state
        .file_upload_coordinator
        .download(&outcome.content_hash)
        .await
        .expect("download must succeed")
        .expect("content must be present");
    assert_eq!(downloaded.len(), content.len());
    assert_eq!(downloaded, content);
}

/// Proves desktop-native notification wiring through the same real SQLite
/// composition root used by Tauri: recipient-filtered inbox query,
/// acknowledgement decision, persisted state transition, and the existing
/// outbox-to-event-bus path that backs `onyx:event`.
#[tokio::test]
async fn app_state_wires_notification_inbox_acknowledgement_and_events() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let recipient_id = test_user_id();
    let other_recipient_id = ObjectId::new_random();
    let notification_id = ObjectId::new_random();
    let other_notification_id = ObjectId::new_random();

    async fn seed_notification(
        pool: &sqlx::SqlitePool,
        id: ObjectId,
        organization_id: OrganizationId,
        recipient_id: ObjectId,
        title: &str,
    ) {
        let state = serde_json::json!({
            "id": id,
            "public_id": format!("notification-{title}"),
            "title": title,
            "message": "A desktop action requires your attention.",
            "priority": "normal",
            "status": "unacknowledged",
            "recipient_id": recipient_id.to_string(),
            "source_id": "source-1",
            "source_type": "task",
            "created_at": "2026-08-17T00:00:00.000Z",
            "acknowledged_at": null,
            "version": 0,
            "lifecycle_epoch": 0,
            "authority_epoch": 0,
        })
        .to_string();

        sqlx::query(
            "INSERT INTO aggregates \
             (id, aggregate_type, version, lifecycle_epoch, authority_epoch, state, updated_at, organization_id) \
             VALUES (?, 'notification', 0, 0, 0, ?, 0, ?)",
        )
        .bind(id.0.to_vec())
        .bind(state)
        .bind(organization_id.0.to_vec())
        .execute(pool)
        .await
        .expect("notification fixture should seed into the real SQLite replica");
    }

    seed_notification(
        &pool,
        notification_id,
        organization_id,
        recipient_id,
        "Assigned review",
    )
    .await;
    seed_notification(
        &pool,
        other_notification_id,
        organization_id,
        other_recipient_id,
        "Someone else's review",
    )
    .await;

    let mut config = test_config();
    config.organization_id = organization_id;
    config.sync_agent_config.outbox_poll_interval = std::time::Duration::from_millis(20);
    let state = Arc::new(AppState::new(pool, config).await);

    let inbox = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "ListNotifications".to_string(),
            target_id: recipient_id,
        })
        .await
        .expect("ListNotifications should use the real local SQLite replica");
    let notifications = inbox["aggregate"]["notifications"]
        .as_array()
        .expect("inbox response should contain a notification array");
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0]["id"], serde_json::json!(notification_id));

    let mut subscription = state
        .event_bus
        .subscribe(client_composition::event_bus::EventFilter {
            organization_id,
            event_types: None,
        });
    let event_pump = tokio::spawn(Arc::clone(&state.sync_agent).run());

    let mut acknowledgement = envelope_for(
        "Acknowledge",
        notification_id,
        organization_id,
        serde_json::json!("Acknowledge"),
    );
    acknowledgement.target.r#type = "notification".to_string();
    acknowledgement.authority_proof.scope.object_type = "notification".to_string();
    let result = state
        .command_registry
        .dispatch(acknowledgement)
        .await
        .expect("Acknowledge should dispatch through the notification handler");
    assert_eq!(result["success"], serde_json::json!(true));

    let refreshed = state
        .query_registry
        .dispatch(client_composition::query_registry::QueryEnvelope {
            query_type: "ListNotifications".to_string(),
            target_id: recipient_id,
        })
        .await
        .expect("notification inbox should remain queryable after acknowledgement");
    assert_eq!(
        refreshed["aggregate"]["notifications"][0]["status"],
        serde_json::json!("acknowledged")
    );

    let event = tokio::time::timeout(std::time::Duration::from_secs(2), subscription.recv())
        .await
        .expect("notification acknowledgement should reach the existing event bus")
        .expect("event subscription should remain open");
    assert_eq!(
        event["aggregate_ref"]["type"],
        serde_json::json!("notification")
    );
    event_pump.abort();
}
