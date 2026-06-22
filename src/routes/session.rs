use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct SessionResponse {
    pub session_id: String,
    pub user_id: String,
}

pub async fn create_session(State(state): State<AppState>) -> Json<SessionResponse> {
    let record = state.sessions.create().await;
    Json(SessionResponse {
        session_id: record.session_id,
        user_id: record.user_id,
    })
}