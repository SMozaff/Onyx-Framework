//! Temporary production-only infrastructure verification for the HF BlobStore.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use query_application::{BlobKey, BlobStore};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::ApiState;

/// Executes one real PUT -> GET -> EXISTS -> DELETE -> GET cycle against the
/// configured BlobStore. Protected by a deployment-only random token and
/// intended to be removed immediately after production verification.
pub async fn verify(headers: HeaderMap, State(state): State<ApiState>) -> (StatusCode, Json<serde_json::Value>) {
    let expected = match std::env::var("ONYX_BLOB_STORE_VERIFY_TOKEN") {
        Ok(value) if !value.is_empty() => value,
        _ => return (StatusCode::NOT_FOUND, Json(json!({"error":"not enabled"}))),
    };
    if headers.get("x-onyx-blob-verify-token").and_then(|v| v.to_str().ok()) != Some(expected.as_str()) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error":"unauthorized"})));
    }

    let payload = b"ONYX HF blob-store production verification 2026-10-06";
    let hash = hex::encode(Sha256::digest(payload));
    let key = BlobKey::new(format!("_verification/{hash}"));

    let result = async {
        state.blob_store.put(&key, payload).await?;
        let got = state.blob_store.get(&key).await?;
        if got.as_deref() != Some(payload) {
            anyhow::bail!("GET content mismatch");
        }
        if !state.blob_store.exists(&key).await? {
            anyhow::bail!("EXISTS returned false after PUT");
        }
        state.blob_store.delete(&key).await?;
        if state.blob_store.exists(&key).await? {
            anyhow::bail!("EXISTS returned true after DELETE");
        }
        if state.blob_store.get(&key).await?.is_some() {
            anyhow::bail!("GET returned content after DELETE");
        }
        Ok::<(), anyhow::Error>(())
    }.await;

    match result {
        Ok(()) => (StatusCode::OK, Json(json!({"ok":true,"backend":"configured_blob_store","operations":["PUT","GET","EXISTS","DELETE","GET-after-delete"],"key":key.0}))),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"ok":false,"error":error.to_string()}))),
    }
}
