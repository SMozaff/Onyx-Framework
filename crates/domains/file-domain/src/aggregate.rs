//! The FileAsset and UploadSession aggregate roots. Source: Part I §4.8.
//!
//! `UploadSession` is an independent aggregate root, not an entity nested
//! inside `FileAsset` — mirrors `communication_domain::Message` being
//! independent of `Conversation` for the same reason: a chunk-by-chunk
//! upload in progress has its own optimistic-concurrency version, and must
//! never contend with `FileAsset`-level operations (access grants,
//! quarantine) on the asset it will eventually become a version of.
//!
//! # Authority (Increment 1 parity)
//! Both aggregates follow `mission_domain::Mission` / `work_domain::Task` /
//! `communication_domain::Conversation`'s established precedent exactly:
//! `decide()` checks only the generic `context.authority.is_authorized(...)`
//! stub. Real per-object checks are ABAC/policy concerns Increment 7 owns
//! (Part I §8), not something this crate invents independently.

use crate::command::{FileAssetCommand, UploadSessionCommand};
use crate::error::{FileAssetError, UploadSessionError};
use crate::event::{FileAssetEvent, UploadSessionEvent};
use crate::state_machine::{FileAssetStatus, UploadSessionStatus};
use crate::value::{
    ContentHash, FileAssetId, FileName, FileVersionRecord, MimeType, UploadSessionId,
    MAX_FILE_SIZE_BYTES,
};
use platform_contracts::{AggregateRoot, DecisionContext};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectVersion, UserId};
use serde::{Deserialize, Serialize};

/// The FileAsset aggregate: identity, access scope, versions, and
/// lifecycle (§4.8.1). Owns *who can access the file and which versions
/// exist*, not the in-flight bytes of an upload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileAsset {
    id: FileAssetId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: FileAssetStatus,
    file_name: String,
    mime_type: String,
    owner_id: UserId,
    access: Vec<UserId>,
    versions: Vec<FileVersionRecord>,
}

impl FileAsset {
    /// Constructs a new FileAsset from a `CreateFileAsset` command. The
    /// creating actor becomes the owner and first authorized user.
    ///
    /// # Errors
    /// Returns [`FileAssetError::MissingField`] if `file_name` or
    /// `mime_type` fail validation, or
    /// [`FileAssetError::InvalidTransition`] if called with any command
    /// other than `CreateFileAsset`.
    pub fn create(
        command: FileAssetCommand,
        context: &DecisionContext,
    ) -> Result<Vec<FileAssetEvent>, FileAssetError> {
        let FileAssetCommand::CreateFileAsset {
            file_name,
            mime_type,
        } = command
        else {
            return Err(FileAssetError::InvalidTransition(
                "FileAsset::create() called with a non-CreateFileAsset command".to_string(),
            ));
        };

        let file_name = FileName::new(file_name).map_err(FileAssetError::MissingField)?;
        let mime_type = MimeType::new(mime_type).map_err(FileAssetError::MissingField)?;

        Ok(vec![FileAssetEvent::FileAssetCreated {
            file_asset_id: FileAssetId(context.generated_id_generator.generate_object_id()),
            file_name: file_name.as_str().to_string(),
            mime_type: mime_type.as_str().to_string(),
            owner_id: context.actor.user_id,
            created_at: context.trusted_now,
        }])
    }

    /// Rehydrates a `FileAsset` by folding its first event
    /// (`FileAssetCreated`).
    ///
    /// # Panics
    /// Panics if `event` is not a `FileAssetCreated` event.
    pub fn from_created_event(event: &FileAssetEvent) -> Self {
        let FileAssetEvent::FileAssetCreated {
            file_asset_id,
            file_name,
            mime_type,
            owner_id,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-FileAssetCreated event");
        };

        Self {
            id: *file_asset_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: FileAssetStatus::Active,
            file_name: file_name.clone(),
            mime_type: mime_type.clone(),
            owner_id: *owner_id,
            access: vec![*owner_id],
            versions: Vec::new(),
        }
    }

    /// The file asset's current status.
    pub fn status(&self) -> FileAssetStatus {
        self.status
    }

    /// The declared file name.
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// The declared MIME type.
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// Who owns (created) this file.
    pub fn owner_id(&self) -> UserId {
        self.owner_id
    }

    /// Users currently granted access.
    pub fn access(&self) -> &[UserId] {
        &self.access
    }

    /// All recorded versions, oldest first.
    pub fn versions(&self) -> &[FileVersionRecord] {
        &self.versions
    }

    fn invalid(&self, command: &str) -> FileAssetError {
        FileAssetError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for FileAsset {
    type Id = FileAssetId;
    type Command = FileAssetCommand;
    type Event = FileAssetEvent;
    type Error = FileAssetError;

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
        if !context.authority.is_authorized("file.command") {
            return Err(FileAssetError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            FileAssetCommand::CreateFileAsset { .. } => Err(FileAssetError::InvalidTransition(
                "CreateFileAsset must be dispatched via FileAsset::create(), not decide()"
                    .to_string(),
            )),

            FileAssetCommand::CreateVersion {
                content_hash,
                size_bytes,
            } => match self.status {
                FileAssetStatus::Active => {
                    if size_bytes > MAX_FILE_SIZE_BYTES {
                        return Err(FileAssetError::FileTooLarge(format!(
                            "{size_bytes} exceeds {MAX_FILE_SIZE_BYTES}"
                        )));
                    }
                    let content_hash =
                        ContentHash::new(content_hash).map_err(FileAssetError::MissingField)?;
                    Ok(vec![FileAssetEvent::FileVersionCreated {
                        content_hash,
                        size_bytes,
                        created_at: context.trusted_now,
                    }])
                }
                FileAssetStatus::Quarantined | FileAssetStatus::Archived => {
                    Err(self.invalid("CreateVersion"))
                }
            },

            FileAssetCommand::GrantFileAccess { user_id } => match self.status {
                FileAssetStatus::Active => {
                    if self.access.contains(&user_id) {
                        return Err(FileAssetError::InvalidAccessState(format!(
                            "{user_id:?} already has access"
                        )));
                    }
                    Ok(vec![FileAssetEvent::FileAccessGranted {
                        user_id,
                        granted_at: context.trusted_now,
                    }])
                }
                FileAssetStatus::Quarantined | FileAssetStatus::Archived => {
                    Err(self.invalid("GrantFileAccess"))
                }
            },

            FileAssetCommand::RevokeFileAccess { user_id } => {
                if !self.access.contains(&user_id) {
                    return Err(FileAssetError::InvalidAccessState(format!(
                        "{user_id:?} does not have access"
                    )));
                }
                Ok(vec![FileAssetEvent::FileAccessRevoked {
                    user_id,
                    revoked_at: context.trusted_now,
                }])
            }

            FileAssetCommand::QuarantineFile { reason } => match self.status {
                FileAssetStatus::Active => Ok(vec![FileAssetEvent::FileQuarantined {
                    reason,
                    quarantined_at: context.trusted_now,
                }]),
                FileAssetStatus::Quarantined | FileAssetStatus::Archived => {
                    Err(self.invalid("QuarantineFile"))
                }
            },

            FileAssetCommand::ArchiveFile { reason } => match self.status {
                FileAssetStatus::Archived => Err(self.invalid("ArchiveFile")),
                FileAssetStatus::Active | FileAssetStatus::Quarantined => {
                    Ok(vec![FileAssetEvent::FileArchived {
                        reason,
                        archived_at: context.trusted_now,
                    }])
                }
            },
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            FileAssetEvent::FileAssetCreated { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            FileAssetEvent::FileVersionCreated {
                content_hash,
                size_bytes,
                created_at,
            } => {
                self.versions.push(FileVersionRecord {
                    content_hash: content_hash.clone(),
                    size_bytes: *size_bytes,
                    created_at: *created_at,
                });
            }
            FileAssetEvent::FileAccessGranted { user_id, .. } => {
                self.access.push(*user_id);
                // Mirrors `Conversation::ConversationMemberAdded` advancing
                // authority_epoch — who may access this file just changed.
                self.authority_epoch = self.authority_epoch.advance();
            }
            FileAssetEvent::FileAccessRevoked { user_id, .. } => {
                self.access.retain(|u| u != user_id);
                self.authority_epoch = self.authority_epoch.advance();
            }
            FileAssetEvent::FileQuarantined { .. } => {
                self.status = FileAssetStatus::Quarantined;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
            FileAssetEvent::FileArchived { .. } => {
                self.status = FileAssetStatus::Archived;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}

/// The UploadSession aggregate: the in-flight, chunk-by-chunk transfer of
/// one file version (§4.8.1) — an independent aggregate root, not an
/// entity of `FileAsset`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadSession {
    id: UploadSessionId,
    version: ObjectVersion,
    lifecycle_epoch: LifecycleEpoch,
    authority_epoch: AuthorityEpoch,
    status: UploadSessionStatus,
    file_asset_id: FileAssetId,
    declared_total_size: u64,
    received_bytes: u64,
    next_chunk_index: u32,
    chunk_hashes: Vec<String>,
}

impl UploadSession {
    /// Constructs a new UploadSession from a `StartUpload` command.
    ///
    /// # Errors
    /// Returns [`UploadSessionError::FileTooLarge`] if `total_size`
    /// exceeds [`MAX_FILE_SIZE_BYTES`], or
    /// [`UploadSessionError::InvalidTransition`] if called with any
    /// command other than `StartUpload`.
    pub fn create(
        command: UploadSessionCommand,
        context: &DecisionContext,
    ) -> Result<Vec<UploadSessionEvent>, UploadSessionError> {
        let UploadSessionCommand::StartUpload {
            file_asset_id,
            total_size,
        } = command
        else {
            return Err(UploadSessionError::InvalidTransition(
                "UploadSession::create() called with a non-StartUpload command".to_string(),
            ));
        };

        if total_size > MAX_FILE_SIZE_BYTES {
            return Err(UploadSessionError::FileTooLarge(format!(
                "declared total {total_size} exceeds {MAX_FILE_SIZE_BYTES}"
            )));
        }

        Ok(vec![UploadSessionEvent::UploadStarted {
            upload_session_id: UploadSessionId(context.generated_id_generator.generate_object_id()),
            file_asset_id,
            declared_total_size: total_size,
            started_at: context.trusted_now,
        }])
    }

    /// Rehydrates an `UploadSession` by folding its first event
    /// (`UploadStarted`).
    ///
    /// # Panics
    /// Panics if `event` is not an `UploadStarted` event.
    pub fn from_created_event(event: &UploadSessionEvent) -> Self {
        let UploadSessionEvent::UploadStarted {
            upload_session_id,
            file_asset_id,
            declared_total_size,
            ..
        } = event
        else {
            panic!("from_created_event called with a non-UploadStarted event");
        };

        Self {
            id: *upload_session_id,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
            status: UploadSessionStatus::InProgress,
            file_asset_id: *file_asset_id,
            declared_total_size: *declared_total_size,
            received_bytes: 0,
            next_chunk_index: 0,
            chunk_hashes: Vec::new(),
        }
    }

    /// The session's current status.
    pub fn status(&self) -> UploadSessionStatus {
        self.status
    }

    /// Which file asset this upload will become a version of.
    pub fn file_asset_id(&self) -> FileAssetId {
        self.file_asset_id
    }

    /// The declared total size in bytes.
    pub fn declared_total_size(&self) -> u64 {
        self.declared_total_size
    }

    /// Bytes received so far.
    pub fn received_bytes(&self) -> u64 {
        self.received_bytes
    }

    /// The next chunk index this session expects.
    pub fn next_chunk_index(&self) -> u32 {
        self.next_chunk_index
    }

    fn invalid(&self, command: &str) -> UploadSessionError {
        UploadSessionError::InvalidTransition(format!("{command} from {:?}", self.status))
    }
}

impl AggregateRoot for UploadSession {
    type Id = UploadSessionId;
    type Command = UploadSessionCommand;
    type Event = UploadSessionEvent;
    type Error = UploadSessionError;

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
        if !context.authority.is_authorized("file.command") {
            return Err(UploadSessionError::Unauthorized(
                "actor lacks required authority".to_string(),
            ));
        }

        match command {
            UploadSessionCommand::StartUpload { .. } => Err(UploadSessionError::InvalidTransition(
                "StartUpload must be dispatched via UploadSession::create(), not decide()"
                    .to_string(),
            )),

            UploadSessionCommand::AppendChunk {
                chunk_index,
                chunk_size,
                chunk_hash,
            } => {
                if self.status != UploadSessionStatus::InProgress {
                    return Err(self.invalid("AppendChunk"));
                }
                if chunk_index != self.next_chunk_index {
                    return Err(UploadSessionError::InvalidChunk(format!(
                        "expected chunk {}, got {chunk_index}",
                        self.next_chunk_index
                    )));
                }
                let projected_total = self.received_bytes + chunk_size;
                // Enforced independently of `StartUpload`'s declared-size
                // check: closes the gap where a client understates its
                // total up front, then keeps appending past the cap.
                if projected_total > MAX_FILE_SIZE_BYTES
                    || projected_total > self.declared_total_size
                {
                    return Err(UploadSessionError::FileTooLarge(format!(
                        "appending {chunk_size} bytes would bring the session to {projected_total}, \
                         exceeding its declared total of {} (cap {MAX_FILE_SIZE_BYTES})",
                        self.declared_total_size
                    )));
                }
                let chunk_hash =
                    ContentHash::new(chunk_hash).map_err(UploadSessionError::InvalidChunk)?;
                Ok(vec![UploadSessionEvent::ChunkAccepted {
                    chunk_index,
                    chunk_size,
                    chunk_hash,
                    accepted_at: context.trusted_now,
                }])
            }

            UploadSessionCommand::FinalizeUpload { final_hash } => {
                if self.status != UploadSessionStatus::InProgress {
                    return Err(self.invalid("FinalizeUpload"));
                }
                if self.received_bytes != self.declared_total_size {
                    return Err(UploadSessionError::Incomplete(format!(
                        "received {} of {} declared bytes",
                        self.received_bytes, self.declared_total_size
                    )));
                }
                let final_hash =
                    ContentHash::new(final_hash).map_err(UploadSessionError::InvalidChunk)?;
                Ok(vec![UploadSessionEvent::UploadFinalized {
                    final_hash,
                    finalized_at: context.trusted_now,
                }])
            }
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            UploadSessionEvent::UploadStarted { .. } => {
                // Handled by `from_created_event`; no-op on replay.
            }
            UploadSessionEvent::ChunkAccepted {
                chunk_index,
                chunk_size,
                chunk_hash,
                ..
            } => {
                self.received_bytes += chunk_size;
                self.next_chunk_index = chunk_index + 1;
                self.chunk_hashes.push(chunk_hash.as_str().to_string());
            }
            UploadSessionEvent::UploadFinalized { .. } => {
                self.status = UploadSessionStatus::Finalized;
                self.lifecycle_epoch = self.lifecycle_epoch.advance();
            }
        }
        self.version = self.version.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        active_file_asset, in_progress_upload_session, test_context, test_user_id,
    };

    // ---- FileAsset -------------------------------------------------------

    #[test]
    fn create_file_asset_makes_creator_owner_and_first_access() {
        let ctx = test_context();
        let events = FileAsset::create(
            FileAssetCommand::CreateFileAsset {
                file_name: "report.pdf".to_string(),
                mime_type: "application/pdf".to_string(),
            },
            &ctx,
        )
        .expect("create must succeed");
        let asset = FileAsset::from_created_event(&events[0]);

        assert_eq!(asset.status(), FileAssetStatus::Active);
        assert_eq!(asset.owner_id(), ctx.actor.user_id);
        assert_eq!(asset.access(), &[ctx.actor.user_id]);
        assert_eq!(asset.file_name(), "report.pdf");
        assert_eq!(asset.version(), ObjectVersion::INITIAL);
    }

    #[test]
    fn create_file_asset_rejects_empty_name() {
        let ctx = test_context();
        let result = FileAsset::create(
            FileAssetCommand::CreateFileAsset {
                file_name: "   ".to_string(),
                mime_type: "application/pdf".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(FileAssetError::MissingField(_))));
    }

    #[test]
    fn file_asset_create_called_with_wrong_command_errors() {
        let ctx = test_context();
        let result = FileAsset::create(FileAssetCommand::ArchiveFile { reason: None }, &ctx);
        assert!(matches!(result, Err(FileAssetError::InvalidTransition(_))));
    }

    #[test]
    fn create_version_from_active_succeeds() {
        let asset = active_file_asset();
        let ctx = test_context();

        let events = asset
            .decide(
                FileAssetCommand::CreateVersion {
                    content_hash: "abc123".to_string(),
                    size_bytes: 1024,
                },
                &ctx,
            )
            .expect("CreateVersion must succeed from Active");
        let mut updated = asset.clone();
        for e in &events {
            updated.apply(e);
        }

        assert_eq!(updated.versions().len(), 1);
        assert_eq!(updated.versions()[0].size_bytes, 1024);
    }

    #[test]
    fn create_version_over_cap_is_rejected() {
        let asset = active_file_asset();
        let ctx = test_context();

        let result = asset.decide(
            FileAssetCommand::CreateVersion {
                content_hash: "abc123".to_string(),
                size_bytes: MAX_FILE_SIZE_BYTES + 1,
            },
            &ctx,
        );
        assert!(matches!(result, Err(FileAssetError::FileTooLarge(_))));
    }

    #[test]
    fn create_version_from_quarantined_is_rejected() {
        let asset = active_file_asset();
        let ctx = test_context();
        let mut quarantined = asset.clone();
        for e in asset
            .decide(
                FileAssetCommand::QuarantineFile {
                    reason: "pending scan".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            quarantined.apply(&e);
        }

        let result = quarantined.decide(
            FileAssetCommand::CreateVersion {
                content_hash: "abc123".to_string(),
                size_bytes: 10,
            },
            &ctx,
        );
        assert!(matches!(result, Err(FileAssetError::InvalidTransition(_))));
    }

    #[test]
    fn grant_file_access_succeeds_and_advances_authority_epoch() {
        let asset = active_file_asset();
        let ctx = test_context();
        let new_user = test_user_id();

        let events = asset
            .decide(
                FileAssetCommand::GrantFileAccess { user_id: new_user },
                &ctx,
            )
            .expect("GrantFileAccess must succeed from Active");
        let mut updated = asset.clone();
        for e in &events {
            updated.apply(e);
        }

        assert!(updated.access().contains(&new_user));
        assert_eq!(updated.authority_epoch(), AuthorityEpoch::INITIAL.advance());
    }

    #[test]
    fn grant_file_access_duplicate_is_rejected() {
        let asset = active_file_asset();
        let ctx = test_context();
        let owner = asset.owner_id();

        let result = asset.decide(FileAssetCommand::GrantFileAccess { user_id: owner }, &ctx);
        assert!(matches!(result, Err(FileAssetError::InvalidAccessState(_))));
    }

    #[test]
    fn revoke_file_access_succeeds() {
        let asset = active_file_asset();
        let ctx = test_context();
        let new_user = test_user_id();
        let mut granted = asset.clone();
        for e in asset
            .decide(
                FileAssetCommand::GrantFileAccess { user_id: new_user },
                &ctx,
            )
            .unwrap()
        {
            granted.apply(&e);
        }
        assert!(granted.access().contains(&new_user));

        let events = granted
            .decide(
                FileAssetCommand::RevokeFileAccess { user_id: new_user },
                &ctx,
            )
            .expect("RevokeFileAccess must succeed");
        let mut revoked = granted.clone();
        for e in &events {
            revoked.apply(e);
        }
        assert!(!revoked.access().contains(&new_user));
    }

    #[test]
    fn revoke_file_access_never_granted_is_rejected() {
        let asset = active_file_asset();
        let ctx = test_context();
        let result = asset.decide(
            FileAssetCommand::RevokeFileAccess {
                user_id: test_user_id(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(FileAssetError::InvalidAccessState(_))));
    }

    #[test]
    fn quarantine_from_active_succeeds() {
        let asset = active_file_asset();
        let ctx = test_context();
        let before_epoch = asset.lifecycle_epoch();

        let events = asset
            .decide(
                FileAssetCommand::QuarantineFile {
                    reason: "suspicious scan result".to_string(),
                },
                &ctx,
            )
            .expect("QuarantineFile must succeed from Active");
        let mut quarantined = asset.clone();
        for e in &events {
            quarantined.apply(e);
        }

        assert_eq!(quarantined.status(), FileAssetStatus::Quarantined);
        assert_eq!(quarantined.lifecycle_epoch(), before_epoch.advance());
    }

    #[test]
    fn quarantine_from_quarantined_is_rejected() {
        let asset = active_file_asset();
        let ctx = test_context();
        let mut quarantined = asset.clone();
        for e in asset
            .decide(
                FileAssetCommand::QuarantineFile {
                    reason: "x".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            quarantined.apply(&e);
        }

        let result = quarantined.decide(
            FileAssetCommand::QuarantineFile {
                reason: "again".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(FileAssetError::InvalidTransition(_))));
    }

    #[test]
    fn archive_from_active_succeeds() {
        let asset = active_file_asset();
        let ctx = test_context();

        let events = asset
            .decide(FileAssetCommand::ArchiveFile { reason: None }, &ctx)
            .expect("ArchiveFile must succeed from Active");
        let mut archived = asset.clone();
        for e in &events {
            archived.apply(e);
        }
        assert_eq!(archived.status(), FileAssetStatus::Archived);
    }

    #[test]
    fn archive_from_quarantined_succeeds() {
        let asset = active_file_asset();
        let ctx = test_context();
        let mut quarantined = asset.clone();
        for e in asset
            .decide(
                FileAssetCommand::QuarantineFile {
                    reason: "x".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            quarantined.apply(&e);
        }

        let result = quarantined.decide(FileAssetCommand::ArchiveFile { reason: None }, &ctx);
        assert!(
            result.is_ok(),
            "a quarantined file must still be archivable"
        );
    }

    #[test]
    fn archive_from_archived_is_rejected() {
        let asset = active_file_asset();
        let ctx = test_context();
        let mut archived = asset.clone();
        for e in asset
            .decide(FileAssetCommand::ArchiveFile { reason: None }, &ctx)
            .unwrap()
        {
            archived.apply(&e);
        }

        let result = archived.decide(FileAssetCommand::ArchiveFile { reason: None }, &ctx);
        assert!(matches!(result, Err(FileAssetError::InvalidTransition(_))));
    }

    // ---- UploadSession -----------------------------------------------------

    #[test]
    fn start_upload_succeeds_within_cap() {
        let ctx = test_context();
        let events = UploadSession::create(
            UploadSessionCommand::StartUpload {
                file_asset_id: FileAssetId::new_random(),
                total_size: 2048,
            },
            &ctx,
        )
        .expect("create must succeed");
        let session = UploadSession::from_created_event(&events[0]);

        assert_eq!(session.status(), UploadSessionStatus::InProgress);
        assert_eq!(session.declared_total_size(), 2048);
        assert_eq!(session.received_bytes(), 0);
    }

    #[test]
    fn start_upload_over_cap_is_rejected() {
        let ctx = test_context();
        let result = UploadSession::create(
            UploadSessionCommand::StartUpload {
                file_asset_id: FileAssetId::new_random(),
                total_size: MAX_FILE_SIZE_BYTES + 1,
            },
            &ctx,
        );
        assert!(matches!(result, Err(UploadSessionError::FileTooLarge(_))));
    }

    #[test]
    fn start_upload_at_exactly_the_cap_succeeds() {
        let ctx = test_context();
        let result = UploadSession::create(
            UploadSessionCommand::StartUpload {
                file_asset_id: FileAssetId::new_random(),
                total_size: MAX_FILE_SIZE_BYTES,
            },
            &ctx,
        );
        assert!(
            result.is_ok(),
            "the cap itself must be an allowed size, not just below it"
        );
    }

    #[test]
    fn upload_session_create_called_with_wrong_command_errors() {
        let ctx = test_context();
        let result = UploadSession::create(
            UploadSessionCommand::FinalizeUpload {
                final_hash: "x".to_string(),
            },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(UploadSessionError::InvalidTransition(_))
        ));
    }

    #[test]
    fn append_chunk_in_order_succeeds() {
        let session = in_progress_upload_session(1000);
        let ctx = test_context();

        let events = session
            .decide(
                UploadSessionCommand::AppendChunk {
                    chunk_index: 0,
                    chunk_size: 400,
                    chunk_hash: "hash0".to_string(),
                },
                &ctx,
            )
            .expect("AppendChunk must succeed in order");
        let mut updated = session.clone();
        for e in &events {
            updated.apply(e);
        }

        assert_eq!(updated.received_bytes(), 400);
        assert_eq!(updated.next_chunk_index(), 1);
    }

    #[test]
    fn append_chunk_out_of_order_is_rejected() {
        let session = in_progress_upload_session(1000);
        let ctx = test_context();

        let result = session.decide(
            UploadSessionCommand::AppendChunk {
                chunk_index: 1,
                chunk_size: 400,
                chunk_hash: "hash1".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(UploadSessionError::InvalidChunk(_))));
    }

    #[test]
    fn append_chunk_duplicate_index_is_rejected() {
        let session = in_progress_upload_session(1000);
        let ctx = test_context();
        let mut advanced = session.clone();
        for e in session
            .decide(
                UploadSessionCommand::AppendChunk {
                    chunk_index: 0,
                    chunk_size: 400,
                    chunk_hash: "hash0".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            advanced.apply(&e);
        }

        let result = advanced.decide(
            UploadSessionCommand::AppendChunk {
                chunk_index: 0,
                chunk_size: 400,
                chunk_hash: "hash0-again".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(UploadSessionError::InvalidChunk(_))));
    }

    #[test]
    fn append_chunk_exceeding_declared_total_is_rejected() {
        let session = in_progress_upload_session(1000);
        let ctx = test_context();

        let result = session.decide(
            UploadSessionCommand::AppendChunk {
                chunk_index: 0,
                chunk_size: 1001,
                chunk_hash: "hash0".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(UploadSessionError::FileTooLarge(_))));
    }

    #[test]
    fn append_chunk_after_finalized_is_rejected() {
        let session = in_progress_upload_session(400);
        let ctx = test_context();
        let mut finalized = session.clone();
        for e in session
            .decide(
                UploadSessionCommand::AppendChunk {
                    chunk_index: 0,
                    chunk_size: 400,
                    chunk_hash: "hash0".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            finalized.apply(&e);
        }
        for e in finalized
            .clone()
            .decide(
                UploadSessionCommand::FinalizeUpload {
                    final_hash: "final".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            finalized.apply(&e);
        }
        assert_eq!(finalized.status(), UploadSessionStatus::Finalized);

        let result = finalized.decide(
            UploadSessionCommand::AppendChunk {
                chunk_index: 1,
                chunk_size: 1,
                chunk_hash: "late".to_string(),
            },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(UploadSessionError::InvalidTransition(_))
        ));
    }

    #[test]
    fn finalize_before_all_bytes_received_is_rejected() {
        let session = in_progress_upload_session(1000);
        let ctx = test_context();
        let mut partial = session.clone();
        for e in session
            .decide(
                UploadSessionCommand::AppendChunk {
                    chunk_index: 0,
                    chunk_size: 400,
                    chunk_hash: "hash0".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            partial.apply(&e);
        }

        let result = partial.decide(
            UploadSessionCommand::FinalizeUpload {
                final_hash: "final".to_string(),
            },
            &ctx,
        );
        assert!(matches!(result, Err(UploadSessionError::Incomplete(_))));
    }

    #[test]
    fn finalize_once_all_bytes_received_succeeds() {
        let session = in_progress_upload_session(400);
        let ctx = test_context();
        let mut complete = session.clone();
        for e in session
            .decide(
                UploadSessionCommand::AppendChunk {
                    chunk_index: 0,
                    chunk_size: 400,
                    chunk_hash: "hash0".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            complete.apply(&e);
        }

        let events = complete
            .decide(
                UploadSessionCommand::FinalizeUpload {
                    final_hash: "final-hash".to_string(),
                },
                &ctx,
            )
            .expect("FinalizeUpload must succeed once all bytes are received");
        let mut finalized = complete.clone();
        for e in &events {
            finalized.apply(e);
        }
        assert_eq!(finalized.status(), UploadSessionStatus::Finalized);
    }

    #[test]
    fn finalize_twice_is_rejected() {
        let session = in_progress_upload_session(400);
        let ctx = test_context();
        let mut finalized = session.clone();
        for e in session
            .decide(
                UploadSessionCommand::AppendChunk {
                    chunk_index: 0,
                    chunk_size: 400,
                    chunk_hash: "hash0".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            finalized.apply(&e);
        }
        for e in finalized
            .clone()
            .decide(
                UploadSessionCommand::FinalizeUpload {
                    final_hash: "final".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            finalized.apply(&e);
        }

        let result = finalized.decide(
            UploadSessionCommand::FinalizeUpload {
                final_hash: "again".to_string(),
            },
            &ctx,
        );
        assert!(matches!(
            result,
            Err(UploadSessionError::InvalidTransition(_))
        ));
    }

    #[test]
    fn version_advances_by_exactly_one_per_applied_event() {
        let session = in_progress_upload_session(1000);
        let ctx = test_context();
        let start = session.version();

        let mut s = session.clone();
        for e in session
            .decide(
                UploadSessionCommand::AppendChunk {
                    chunk_index: 0,
                    chunk_size: 100,
                    chunk_hash: "hash0".to_string(),
                },
                &ctx,
            )
            .unwrap()
        {
            s.apply(&e);
        }
        assert_eq!(s.version(), start.next());
    }
}
