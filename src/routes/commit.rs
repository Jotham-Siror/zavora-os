use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct CommitRequest {
    pub card_title: String,
    pub action_label: String,
}

#[derive(Serialize)]
pub struct CommitResponse {
    pub status: &'static str,
    pub action: String,
    pub message: String,
}

pub async fn commit_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(body): Json<CommitRequest>,
) -> Result<Json<CommitResponse>, Response> {
    let client_key = format!("commit:{session_id}");
    state
        .awp
        .check(&headers, &client_key, "commit_action")
        .await
        .map_err(|r| r)?;

    let Some(record) = state.sessions.get(&session_id).await else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };

    let scenario = record.scenario.as_deref().unwrap_or("");
    let message = match (scenario, body.action_label.to_lowercase().as_str()) {
        ("morning", label) if label.contains("draft") => {
            "Draft replies queued via inbox agent (connect email MCP for live drafts)."
                .into()
        }
        ("deck", label) if label.contains("save") || label.contains("open") => {
            "Artifact link preserved on card.".into()
        }
        _ => format!("Recorded commit: {}", body.action_label),
    };

    Ok(Json(CommitResponse {
        status: "ok",
        action: body.action_label,
        message,
    }))
}