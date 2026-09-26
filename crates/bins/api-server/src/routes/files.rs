//! File download route for the Server-driven clients (web-ui, PWA
//! ObserverClient). MIGRATION_PLAN Phase 1.2 addition.
//!
//! # How this route is a read, not a mutation
//! `GET /api/files/:content_hash` returns stored file bytes without
//! changing any state, so it is gated by the read-class capability
//! ceiling `can_download_files` — the same ceiling the desktop
//! download path (`FileUploadCoordinator::download`) is subject to
//! through the raw filesystem it already has. `MobileObserver` grants
//! this flag (every observer `can_read_*`/`can_download_files` is
//! true per ONYX-MOB-01 §8); the check still exists so a future
//! client class that should not download is refused here without
//! needing a new route.
//!
//! # Content addressing and tenant scoping (disclosed limitation)
//! Blobs are content-addressed by sha256, so the route serves whatever
//! bytes are stored under the requested hash — the hash *is* the
//! tamper-evident identity of the content, and the same hash is recorded
//! on a `FileAsset`'s `FileVersionRecord` at upload time (see
//! `client-composition`'s `FileUploadCoordinator`). This route does not
//! additionally consult the `FileAsset` projections to scope a hash to a
//! caller's organization: api-server has no file-listing route or
//! FileAsset query path yet (Phase 1.2 scope), so there is no first-party
//! way for an observer to learn another org's hashes through HTTP anyway.
//! A tenant-scoped FileAsset lookup is flagged as the follow-up when the
//! PWA FileList view lands (MIGRATION_PLAN Phase 3.1). The capability
//! ceiling and a caller's existing read access to the content are the
//! only gates for now.
//!
//! # Error codes
//! - `400 INVALID_CONTENT_HASH` — hash is not lowercase hex (the wire
//!   shape `file_domain::value::ContentHash` validates, matching
//!   `LocalBlobStore`'s own key format).
//! - `404 FILE_NOT_FOUND` — no blob is stored for a well-formed hash.
//! - `401`/`403` via the ordinary auth + capability paths.

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use query_application::BlobKey;
use serde_json::json;

use super::{authenticate_headers, client_type::require_capability, ApiError, ApiState};

/// Returns the stored file bytes for `content_hash`, or the documented
/// error codes above.
pub async fn download_file(
    State(state): State<ApiState>,
    Path(content_hash): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let actor = authenticate_headers(&state, &headers).await?;
    require_capability(&actor, |c| c.can_download_files, "download_files")?;

    if !is_lowercase_hex(&content_hash) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_CONTENT_HASH",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation_id,
            json!({"message": "content_hash must be a lowercase hex string"}),
        ));
    }

    let content = state
        .blob_store
        .get(&BlobKey::new(content_hash.clone()))
        .await
        .map_err(|error| {
            tracing::error!(content_hash = %content_hash, error = %error, "blob store read failure");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "BLOB_STORE_UNAVAILABLE",
                "INFRASTRUCTURE",
                "TRANSIENT",
                correlation_id.clone(),
                json!({}),
            )
        })?;

    match content {
        Some(bytes) => Ok((
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (header::CONTENT_LENGTH, bytes.len().to_string()),
            ],
            bytes,
        )
            .into_response()),
        None => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "FILE_NOT_FOUND",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation_id,
            json!({"message": "no stored content for this hash"}),
        )),
    }
}

/// Whether `candidate` matches the wire format `ContentHash` validates
/// (also enforced by `LocalBlobStore`): a non-empty lowercase hex string.
fn is_lowercase_hex(candidate: &str) -> bool {
    !candidate.is_empty()
        && candidate
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
