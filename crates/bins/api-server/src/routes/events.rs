use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value;

use super::{validate_token, ApiError, ApiState};

#[derive(Debug, Deserialize)]
pub struct WebSocketAuth {
    pub token: String,
}

pub async fn websocket_route(
    ws: WebSocketUpgrade,
    Query(params): Query<WebSocketAuth>,
    State(state): State<ApiState>,
) -> Result<Response, ApiError> {
    let claims = validate_token(&state, &params.token, "access").await?;
    Ok(ws.on_upgrade(move |socket| serve_socket(socket, state, claims.organization_id)))
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
                        if sender.send(Message::Text(value.to_string().into())).await.is_err() { break; }
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
