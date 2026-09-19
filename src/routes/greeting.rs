use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::greeting::{self, GreetingPayload};
use crate::state::AppState;

#[derive(Deserialize, Default)]
pub struct GreetingQuery {
    /// When given, the greeting uses this session's user profile (home location) from memory.
    pub session_id: Option<String>,
}

pub async fn get_greeting(
    State(state): State<AppState>,
    Query(q): Query<GreetingQuery>,
) -> Json<GreetingPayload> {
    let mut home_location = None;
    if let Some(sid) = q.session_id.as_deref() {
        if let Some(rec) = state.sessions.get(sid).await {
            home_location = state.memory.profile(&rec.user_id, "home_location").await;
        }
    }
    let payload = greeting::compose(
        state.greeting_runner.as_ref(),
        state.morning_mcp.as_deref(),
        &state.brand_greeting_body,
        state.brand_tone.as_deref(),
        home_location.as_deref(),
    )
    .await;
    Json(payload)
}