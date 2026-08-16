//! Deterministic test fixtures for `file-domain`'s test suites.
//!
//! Deliberately self-contained rather than reusing another domain crate's
//! fixtures — same rationale as
//! `communication_domain::test_support`'s doc comment: no genuine business
//! dependency exists between File and any sibling domain, so a small
//! amount of duplication here is the correct trade, not an oversight.

use platform_contracts::{DecisionContext, IdGenerator};
use platform_kernel::{
    ActorContext, EventId, ObjectId, OperationId, PolicyDecisionSet, Timestamp, UserId,
    VerifiedAuthority,
};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::aggregate::{FileAsset, UploadSession};
use crate::command::{FileAssetCommand, UploadSessionCommand};
use crate::value::FileAssetId;

/// A deterministic, counter-based `IdGenerator` for tests. See
/// `mission_domain::test_support::DeterministicIdGenerator`'s doc comment
/// for the full rationale (reproducibility over randomness; `Sync` via
/// `AtomicU64` rather than `Cell`) — identical reasoning applies here.
struct DeterministicIdGenerator {
    counter: AtomicU64,
}

impl DeterministicIdGenerator {
    fn new_seeded(seed: u64) -> Self {
        Self {
            counter: AtomicU64::new(seed),
        }
    }

    fn next_bytes(&self) -> [u8; 16] {
        let value = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut bytes = [0u8; 16];
        bytes[8..].copy_from_slice(&value.to_be_bytes());
        bytes
    }
}

impl IdGenerator for DeterministicIdGenerator {
    fn generate_object_id(&self) -> ObjectId {
        ObjectId(self.next_bytes())
    }
    fn generate_operation_id(&self) -> OperationId {
        OperationId(self.next_bytes())
    }
    fn generate_event_id(&self) -> EventId {
        EventId(self.next_bytes())
    }
}

/// Process-wide seed counter, same isolation rationale as
/// `mission_domain::test_support::TEST_CONTEXT_SEED`: concurrently-running
/// tests must never collide on a generated `ObjectId`.
static TEST_CONTEXT_SEED: AtomicU64 = AtomicU64::new(1);

/// Builds a `DecisionContext` suitable for deterministic tests.
pub fn test_context() -> DecisionContext {
    let seed = TEST_CONTEXT_SEED.fetch_add(1_000_000, Ordering::SeqCst);
    let org = ObjectId::new_random();
    DecisionContext {
        actor: ActorContext {
            user_id: ObjectId::new_random(),
            device_id: ObjectId::new_random(),
            organization_id: org,
        },
        authority: VerifiedAuthority,
        trusted_now: Timestamp::from_nanos(1_700_000_000_000_000_000),
        policy_outcomes: PolicyDecisionSet,
        generated_id_generator: Box::new(DeterministicIdGenerator::new_seeded(seed)),
    }
}

/// Generates a fresh random `UserId` for use as an actor/grantee in tests.
pub fn test_user_id() -> UserId {
    ObjectId::new_random()
}

/// A freshly created, `Active` file asset owned by its creator.
pub fn active_file_asset() -> FileAsset {
    let ctx = test_context();
    let events = FileAsset::create(
        FileAssetCommand::CreateFileAsset {
            file_name: "sprint-plan.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
        },
        &ctx,
    )
    .expect("create must succeed");
    FileAsset::from_created_event(&events[0])
}

/// A freshly started, `InProgress` upload session declaring
/// `total_size` bytes for a fresh file asset.
pub fn in_progress_upload_session(total_size: u64) -> UploadSession {
    let ctx = test_context();
    let events = UploadSession::create(
        UploadSessionCommand::StartUpload {
            file_asset_id: FileAssetId::new_random(),
            total_size,
        },
        &ctx,
    )
    .expect("create must succeed");
    UploadSession::from_created_event(&events[0])
}
