use axum::{extract::State, Json};
use serde::Serialize;

use adk_session::{CreateRequest, SessionService};

use crate::state::AppState;

#[derive(Serialize)]
pub struct SessionResponse {
    pub session_id: String,
    pub user_id: String,
}

pub async fn create_session(State(state): State<AppState>) -> Json<SessionResponse> {
    let record = state.sessions.create().await;

    let _ = state
        .session_service
        .create(CreateRequest {
            app_name: "zavora-os".into(),
            user_id: record.user_id.clone(),
            session_id: Some(record.session_id.clone()),
            state: Default::default(),
        })
        .await;

    Json(SessionResponse {
        session_id: record.session_id,
        user_id: record.user_id,
    })
}