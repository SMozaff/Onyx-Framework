//! `FileUploadCoordinator` — Phase 1 (Desktop & Web Completion)
//! addition, ties `file-domain`'s pure metadata commands to a real
//! [`BlobStore`] so a Desktop upload actually persists file content
//! somewhere, not just its hash/size (see `query_application::BlobStore`'s
//! doc comment for the gap this closes — confirmed to exist before this
//! was built, not assumed).
//!
//! # Why this isn't just more `CommandRegistry` entries
//! Every other `CreationHandler`/`DecisionHandler` in this crate takes a
//! `serde_json::Value` payload from a `CommandEnvelope` and dispatches
//! exactly one domain command. A real upload is a multi-step sequence
//! (`StartUpload` → N × `AppendChunk` → `FinalizeUpload` → `CreateVersion`),
//! each step depending on the previous step's resulting version/epoch,
//! and each `AppendChunk` needing the actual chunk bytes (which
//! `file-domain`'s commands deliberately never carry — see that crate's
//! own doc comments). Threading raw bytes and a multi-step loop through
//! the generic JSON command envelope path would be awkward and add
//! nothing the caller doesn't already have (desktop-shell reads the
//! source file directly). This coordinator is a dedicated, typed entry
//! point instead, reusing `api_server::handle_command` for each
//! decision-path step so the version/epoch-conflict and event-envelope
//! logic is not duplicated.

use std::sync::Arc;

use platform_contracts::AggregateRoot;
use platform_kernel::{ActorContext, CorrelationId, ObjectId, OperationId, VectorClock};
use query_application::{BlobKey, BlobStore, Repository, UnitOfWorkFactory};
use sha2::{Digest, Sha256};

use crate::handlers::creation_decision_context;

/// One chunk's worth of upload — kept small and deliberate rather than
/// exposing raw byte-slicing parameters, since the "how big is a chunk"
/// policy belongs to the caller (desktop-shell), not this coordinator.
const CHUNK_SIZE_BYTES: usize = 4 * 1024 * 1024; // 4 MiB.

#[derive(Debug, thiserror::Error)]
pub enum FileUploadError {
    #[error("domain error during creation: {0}")]
    Creation(#[from] crate::command_registry::CreationError),
    #[error(transparent)]
    Decision(#[from] api_server::CommandError),
    #[error("blob storage error: {0}")]
    Blob(#[from] query_application::BlobStoreError),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Coordinates a full file upload: creates the `FileAsset` and
/// `UploadSession` aggregates, chunks the given bytes, writes each chunk
/// to the [`BlobStore`] and records it via `AppendChunk`, finalizes the
/// session, and records the resulting version on the `FileAsset` via
/// `CreateVersion`.
pub struct FileUploadCoordinator {
    file_asset_repo: Arc<dyn Repository>,
    upload_session_repo: Arc<dyn Repository>,
    unit_factory: Arc<dyn UnitOfWorkFactory>,
    blob_store: Arc<dyn BlobStore>,
}

/// The outcome of a completed upload.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UploadOutcome {
    pub file_asset_id: ObjectId,
    pub upload_session_id: ObjectId,
    pub content_hash: String,
    pub size_bytes: u64,
}

impl FileUploadCoordinator {
    pub fn new(
        file_asset_repo: Arc<dyn Repository>,
        upload_session_repo: Arc<dyn Repository>,
        unit_factory: Arc<dyn UnitOfWorkFactory>,
        blob_store: Arc<dyn BlobStore>,
    ) -> Self {
        Self {
            file_asset_repo,
            upload_session_repo,
            unit_factory,
            blob_store,
        }
    }

    /// Uploads `content` as a new file named `file_name`, of MIME type
    /// `mime_type`. Creates a brand-new `FileAsset` for it — this
    /// coordinator does not currently support uploading a new version of
    /// an *existing* `FileAsset` (that would additionally need the
    /// existing asset's id, version, and epochs as input; not built yet,
    /// no Phase 1 UI workflow needs it — flagged, not silently omitted).
    pub async fn upload_new_file(
        &self,
        actor: ActorContext,
        file_name: String,
        mime_type: String,
        content: &[u8],
    ) -> Result<UploadOutcome, FileUploadError> {
        // 1. CreateFileAsset (creation path — no existing aggregate).
        let file_asset_id = self
            .create_file_asset(actor.clone(), file_name, mime_type)
            .await?;

        // 2. StartUpload (creation path).
        let (upload_session_id, mut session_version) = self
            .start_upload(actor.clone(), file_asset_id, content.len() as u64)
            .await?;

        // 3. AppendChunk per chunk (decision path, reusing
        // `api_server::handle_command` so version/epoch tracking and
        // event envelopes are not hand-rolled here).
        // `(0_u32..).zip(...)` rather than `.enumerate()`: `chunk_index`
        // is a `u32` on `AppendChunk` (see `file_domain`'s command), and
        // `enumerate()` yields `usize`, which would need a cast on every
        // iteration.
        for (chunk_index, chunk) in (0_u32..).zip(content.chunks(CHUNK_SIZE_BYTES)) {
            let chunk_hash = hex_sha256(chunk);
            self.blob_store
                .put(&BlobKey::new(chunk_hash.clone()), chunk)
                .await?;

            let command = file_domain::UploadSessionCommand::AppendChunk {
                chunk_index,
                chunk_size: chunk.len() as u64,
                chunk_hash,
            };
            let result = api_server::handle_command::<file_domain::UploadSession, _, _, _>(
                command,
                upload_session_id,
                OperationId::new_random(),
                actor.clone(),
                session_version,
                platform_kernel::LifecycleEpoch::INITIAL,
                platform_kernel::AuthorityEpoch::INITIAL,
                VectorClock::new(),
                CorrelationId::new_random(),
                "upload_session",
                Arc::clone(&self.upload_session_repo),
                Arc::clone(&self.unit_factory),
                self.noop_idempotency_store(),
            )
            .await?;
            session_version = extract_new_version(&result)?;
        }

        // 4. FinalizeUpload (decision path).
        let final_hash = hex_sha256(content);
        let finalize = file_domain::UploadSessionCommand::FinalizeUpload {
            final_hash: final_hash.clone(),
        };
        api_server::handle_command::<file_domain::UploadSession, _, _, _>(
            finalize,
            upload_session_id,
            OperationId::new_random(),
            actor.clone(),
            session_version,
            platform_kernel::LifecycleEpoch::INITIAL,
            platform_kernel::AuthorityEpoch::INITIAL,
            VectorClock::new(),
            CorrelationId::new_random(),
            "upload_session",
            Arc::clone(&self.upload_session_repo),
            Arc::clone(&self.unit_factory),
            self.noop_idempotency_store(),
        )
        .await?;

        // Also store the whole file under its own content hash — chunk
        // hashes above let `LocalBlobStore` reconstruct content
        // chunk-by-chunk, but every current reader (see `Files.tsx`)
        // downloads by the file's overall `content_hash`, so both are
        // written. A future optimization could instead reconstruct the
        // full blob from its chunks on first read and cache it, rather
        // than storing the content twice; not built now since Phase 1
        // has no file large enough (100MB cap) to make the duplication
        // costly, and correctness matters more than this optimization.
        self.blob_store
            .put(&BlobKey::new(final_hash.clone()), content)
            .await?;

        // 5. CreateVersion on the FileAsset (decision path).
        let create_version = file_domain::FileAssetCommand::CreateVersion {
            content_hash: final_hash.clone(),
            size_bytes: content.len() as u64,
        };
        api_server::handle_command::<file_domain::FileAsset, _, _, _>(
            create_version,
            file_asset_id,
            OperationId::new_random(),
            actor,
            platform_kernel::ObjectVersion::INITIAL,
            platform_kernel::LifecycleEpoch::INITIAL,
            platform_kernel::AuthorityEpoch::INITIAL,
            VectorClock::new(),
            CorrelationId::new_random(),
            "file_asset",
            Arc::clone(&self.file_asset_repo),
            Arc::clone(&self.unit_factory),
            self.noop_idempotency_store(),
        )
        .await?;

        Ok(UploadOutcome {
            file_asset_id,
            upload_session_id,
            content_hash: final_hash,
            size_bytes: content.len() as u64,
        })
    }

    /// Downloads a file's content by its `content_hash` (as recorded on
    /// one of its `FileVersionRecord`s — see `file_domain::value`).
    /// Returns `None` if no blob is stored for that hash (e.g. the asset
    /// was created through some other path that never called
    /// `upload_new_file`, or its blob was deleted).
    pub async fn download(&self, content_hash: &str) -> Result<Option<Vec<u8>>, FileUploadError> {
        Ok(self.blob_store.get(&BlobKey::new(content_hash)).await?)
    }

    async fn create_file_asset(
        &self,
        actor: ActorContext,
        file_name: String,
        mime_type: String,
    ) -> Result<ObjectId, FileUploadError> {
        let ctx = creation_decision_context(actor);
        let command = file_domain::FileAssetCommand::CreateFileAsset {
            file_name,
            mime_type,
        };
        let events = file_domain::FileAsset::create(command, &ctx)
            .map_err(|e| crate::command_registry::CreationError::Domain(e.to_string()))?;
        let asset = file_domain::FileAsset::from_created_event(&events[0]);
        let aggregate_state = serde_json::to_value(&asset)?;

        let mut unit = self
            .unit_factory
            .create(actor_org(&ctx))
            .await
            .map_err(|e| crate::command_registry::CreationError::Persistence(e.to_string()))?;
        self.file_asset_repo
            .commit(aggregate_state, &[], &mut *unit)
            .await
            .map_err(|e| crate::command_registry::CreationError::Persistence(e.to_string()))?;
        unit.commit()
            .await
            .map_err(|e| crate::command_registry::CreationError::Persistence(e.to_string()))?;

        Ok(asset.id().0)
    }

    async fn start_upload(
        &self,
        actor: ActorContext,
        file_asset_id: ObjectId,
        total_size: u64,
    ) -> Result<(ObjectId, platform_kernel::ObjectVersion), FileUploadError> {
        let ctx = creation_decision_context(actor);
        let command = file_domain::UploadSessionCommand::StartUpload {
            file_asset_id: file_domain::value::FileAssetId(file_asset_id),
            total_size,
        };
        let events = file_domain::UploadSession::create(command, &ctx)
            .map_err(|e| crate::command_registry::CreationError::Domain(e.to_string()))?;
        let session = file_domain::UploadSession::from_created_event(&events[0]);
        let aggregate_state = serde_json::to_value(&session)?;

        let mut unit = self
            .unit_factory
            .create(actor_org(&ctx))
            .await
            .map_err(|e| crate::command_registry::CreationError::Persistence(e.to_string()))?;
        self.upload_session_repo
            .commit(aggregate_state, &[], &mut *unit)
            .await
            .map_err(|e| crate::command_registry::CreationError::Persistence(e.to_string()))?;
        unit.commit()
            .await
            .map_err(|e| crate::command_registry::CreationError::Persistence(e.to_string()))?;

        Ok((session.id().0, session.version()))
    }

    /// A fresh, empty in-memory `IdempotencyStore` per call. This
    /// coordinator's multi-step flow already has its own retry/failure
    /// semantics at the coordinator level (a failed step simply returns
    /// an error to the caller, who can retry the whole
    /// `upload_new_file` call) — reusing `AppState`'s shared
    /// process-lifetime idempotency store here would be actively wrong,
    /// since each `handle_command` call within one upload uses a fresh
    /// `OperationId` by design (they are genuinely different operations,
    /// not retries of the same one), so idempotency caching across them
    /// would never hit anyway.
    fn noop_idempotency_store(&self) -> Arc<dyn query_application::IdempotencyStore> {
        Arc::new(EmptyIdempotencyStore)
    }
}

fn actor_org(ctx: &platform_contracts::DecisionContext) -> platform_kernel::OrganizationId {
    ctx.actor.organization_id
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn extract_new_version(
    result: &serde_json::Value,
) -> Result<platform_kernel::ObjectVersion, FileUploadError> {
    let raw = result
        .get("new_version")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(serde_json::from_value(raw)?)
}

/// Always-empty `IdempotencyStore`, used only by
/// [`FileUploadCoordinator`] — see `noop_idempotency_store`'s doc
/// comment for why a real, shared store is deliberately not used here.
struct EmptyIdempotencyStore;

#[async_trait::async_trait]
impl query_application::IdempotencyStore for EmptyIdempotencyStore {
    async fn get(
        &self,
        _operation_id: &OperationId,
    ) -> Result<Option<serde_json::Value>, query_application::IdempotencyError> {
        Ok(None)
    }

    async fn put(
        &self,
        _operation_id: OperationId,
        _result: serde_json::Value,
    ) -> Result<(), query_application::IdempotencyError> {
        Ok(())
    }
}
