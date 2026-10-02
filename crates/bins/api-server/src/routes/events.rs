use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{
        header::SEC_WEBSOCKET_PROTOCOL,
        HeaderMap, StatusCode,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;

use super::{validate_token, ApiError, ApiState};

/// Fixed WebSocket subprotocol negotiated for the authenticated event stream.
///
/// The access token is carried as a second requested subprotocol rather than
/// in the URL query string. The server only echoes this fixed protocol, so the
/// bearer token is not copied into the upgrade response.
const WS_AUTH_PROTOCOL: &str = "onyx-bearer";

fn bearer_token_from_protocols(headers: &HeaderMap) -> Option<&str> {
    let mut found_auth_protocol = false;
    let mut token = None;

    for header_value in headers.get_all(SEC_WEBSOCKET_PROTOCOL).iter() {
        let protocols = header_value.to_str().ok()?.split(',');
        for protocol in protocols.map(str::trim).filter(|p| !p.is_empty()) {
            if protocol == WS_AUTH_PROTOCOL {
                found_auth_protocol = true;
            } else if token.is_none() {
                token = Some(protocol);
            }
        }
    }

    found_auth_protocol.then_some(token?).filter(|value| !value.is_empty())
}

pub async fn websocket_route(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<ApiState>,
) -> Result<Response, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let token = bearer_token_from_protocols(&headers)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "AUTHORITY",
                "NON_RETRYABLE",
                correlation_id.clone(),
                serde_json::json!({
                    "message": "WebSocket authentication requires the onyx-bearer subprotocol and access token"
                }),
            )
        })?;
    let claims = validate_token(&state, token, "access").await?;

    Ok(ws
        .protocols([WS_AUTH_PROTOCOL])
        .on_upgrade(move |socket| serve_socket(socket, state, claims.organization_id)))
}

#[derive(Debug, Default)]
struct SubscriptionFilter {
    organization_id: String,
    event_types: Vec<String>,
    aggregate_ids: Vec<String>,
}

async fn serve_socket(socket: WebSocket, state: ApiState, authenticated_org: String) {
    let (mut sender, mut receiver) = socket.split();
    let mut bus = state.events.subscribe();
    let mut filter = SubscriptionFilter {
        organization_id: authenticated_org.clone(),
        ..Default::default()
    };

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
                            if value.get("type").and_then(Value::as_str) == Some("subscribe") {
                                if let Some(requested_org) = value.pointer("/filter/organization_id").and_then(Value::as_str) {
                                    if requested_org != authenticated_org {
                                        let _ = sender.send(Message::Text("{\"type\":\"error\",\"code\":\"TENANT_MISMATCH\"}".into())).await;
                                        break;
                                    }
                                    filter.organization_id = requested_org.to_string();
                                }
                                filter.event_types = string_array(value.pointer("/filter/event_types"));
                                filter.aggregate_ids = string_array(value.pointer("/filter/aggregate_ids"));
                                let _ = sender.send(Message::Text("{\"type\":\"subscribed\"}".into())).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
            event = bus.recv() => {
                match event {
                    Ok(value) if matches_filter(&value, &filter) => {
                        if sender.send(Message::Text(value.to_string())).await.is_err() { break; }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let _ = sender.send(Message::Text("{\"type\":\"resync_required\"}".into())).await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn matches_filter(event: &Value, filter: &SubscriptionFilter) -> bool {
    let org = event
        .pointer("/aggregate_ref/organization_id")
        .and_then(Value::as_str);
    if org != Some(filter.organization_id.as_str()) {
        return false;
    }
    if !filter.event_types.is_empty() {
        let event_type = event
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !filter
            .event_types
            .iter()
            .any(|allowed| allowed == event_type)
        {
            return false;
        }
    }
    if !filter.aggregate_ids.is_empty() {
        let aggregate_id = event
            .pointer("/aggregate_ref/id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !filter
            .aggregate_ids
            .iter()
            .any(|allowed| allowed == aggregate_id)
        {
            return false;
        }
    }
    true
}


#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn extracts_access_token_from_websocket_subprotocols() {
        let mut headers = HeaderMap::new();
        headers.insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("onyx-bearer, access.token"),
        );

        assert_eq!(bearer_token_from_protocols(&headers), Some("access.token"));
    }

    #[test]
    fn rejects_missing_authentication_protocol() {
        let mut headers = HeaderMap::new();
        headers.insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("access.token"),
        );

        assert_eq!(bearer_token_from_protocols(&headers), None);
    }

    #[test]
    fn rejects_empty_token_after_authentication_protocol() {
        let mut headers = HeaderMap::new();
        headers.insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("onyx-bearer"));

        assert_eq!(bearer_token_from_protocols(&headers), None);
    }

    #[test]
    fn handles_multiple_websocket_protocol_header_values() {
        let mut headers = HeaderMap::new();
        headers.append(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("chat"),
        );
        headers.append(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("onyx-bearer"),
        );
        headers.append(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static("access.token"),
        );

        assert_eq!(bearer_token_from_protocols(&headers), Some("chat"));
    }
}
