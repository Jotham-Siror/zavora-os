use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    response::sse::Sse,
    Json,
};
use serde::Deserialize;

use crate::events::mock;
use crate::state::{AppState, SessionStore};

#[derive(Deserialize)]
pub struct IntentRequest {
    pub text: String,
}

pub async fn submit_intent(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<IntentRequest>,
) -> Response {
    if body.text.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "intent text required").into_response();
    }

    if state.sessions.get(&session_id).await.is_none() {
        return (StatusCode::NOT_FOUND, "session not found").into_response();
    }

    Sse::new(mock::stream_intent(&body.text)).into_response()
}

/// A2A stub that forwards intent text to the same mock orchestrator acknowledgement.
pub async fn a2a_intent(
    Extension(sessions): Extension<SessionStore>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let message_id = body
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let intent_text = body
        .get("payload")
        .and_then(|p| {
            p.get("intent")
                .or_else(|| p.get("query"))
                .or_else(|| p.get("text"))
        })
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if intent_text.trim().is_empty() {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "acknowledged",
                "messageId": message_id,
                "note": "no intent in payload.intent|query|text"
            })),
        )
            .into_response();
    }

    let record = sessions.create().await;
    let scenario = mock::pick_scenario(&intent_text);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "acknowledged",
            "messageId": message_id,
            "sessionId": record.session_id,
            "userId": record.user_id,
            "scenario": scenario,
            "intentEndpoint": format!("/api/sessions/{}/intent", record.session_id)
        })),
    )
        .into_response()
}