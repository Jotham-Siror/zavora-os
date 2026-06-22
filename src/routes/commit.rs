use axum::{
    extract::{Path, State},
    http::StatusCode,
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
    Path(session_id): Path<String>,
    Json(body): Json<CommitRequest>,
) -> Result<Json<CommitResponse>, StatusCode> {
    let Some(record) = state.sessions.get(&session_id).await else {
        return Err(StatusCode::NOT_FOUND);
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