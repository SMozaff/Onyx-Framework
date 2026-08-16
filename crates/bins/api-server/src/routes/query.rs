use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{authenticate_headers, ApiError, ApiState, QueryEnvelopeDto};

#[derive(Debug, Deserialize)]
pub struct Base64Envelope {
    pub envelope: String,
}

pub async fn query_route(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(params): Query<Base64Envelope>,
) -> Result<Json<Value>, ApiError> {
    let auth = authenticate_headers(&state, &headers).await?;
    let decoded = URL_SAFE_NO_PAD.decode(&params.envelope).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_QUERY_ENVELOPE",
            "DOMAIN",
            "NON_RETRYABLE",
            uuid::Uuid::new_v4().to_string(),
            json!({"message":"envelope must be base64url JSON"}),
        )
    })?;
    let envelope: QueryEnvelopeDto = serde_json::from_slice(&decoded).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_QUERY_ENVELOPE",
            "DOMAIN",
            "NON_RETRYABLE",
            uuid::Uuid::new_v4().to_string(),
            json!({"message":"decoded envelope is not valid JSON"}),
        )
    })?;
    if let Some(error) = super::test_mode_error(&headers, &envelope.query_id) {
        return Err(error);
    }
    if envelope.organization_id != auth.organization_id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "TENANT_MISMATCH",
            "AUTHORITY",
            "NON_RETRYABLE",
            envelope.query_id,
            json!({}),
        ));
    }
    let viewer = super::parse_object_id(&auth.user_id).ok();
    let result = crate::query_handler::execute_query(
        &state.projection_pool,
        &envelope.query_type,
        &envelope.organization_id,
        &envelope.filters,
        envelope.limit,
        envelope.cursor.as_deref(),
        viewer,
    )
    .await
    .map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "QUERY_FAILED",
            "INFRASTRUCTURE",
            "TRANSIENT",
            envelope.query_id.clone(),
            json!({"message": e.to_string()}),
        )
    })?;

    Ok(Json(json!({
        "query_id": envelope.query_id,
        "data": result.data,
        "has_more": result.has_more,
        "next_cursor": result.next_cursor,
        "total_count": result.total_count,
        "freshness": result.freshness
    })))
}
