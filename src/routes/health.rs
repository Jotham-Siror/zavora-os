use axum::{extract::State, Json};
use serde_json::json;

use crate::state::AppState;

pub async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "zavora-os",
        "milestone": "M10",
        "voice_enabled": state.voice.enabled
    }))
}