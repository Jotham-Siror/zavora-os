use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use adk_awp::error_response::AwpErrorResponse;
use adk_awp::{EventSubscription, EventSubscriptionService};

use crate::awp_gate;
use crate::state::AppState;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubscriptionRequest {
    pub subscriber: String,
    pub callback_url: String,
    pub event_types: Vec<String>,
    pub secret: String,
}

/// `POST /awp/events/subscribe` — gated to `known` trust (JWT or Bearer).
pub async fn subscribe(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateSubscriptionRequest>,
) -> Response {
    let key = awp_gate::client_key(&headers, "awp-subscribe");
    if let Err(resp) = app
        .awp
        .check(&headers, &key, "subscribe_proactive")
        .await
    {
        return resp;
    }

    let subscription = EventSubscription {
        id: Uuid::now_v7(),
        subscriber: body.subscriber,
        callback_url: body.callback_url,
        event_types: body.event_types,
        secret: body.secret,
    };

    match app.event_service.create(subscription.clone()).await {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response(),
        Err(e) => AwpErrorResponse(e).into_response(),
    }
}