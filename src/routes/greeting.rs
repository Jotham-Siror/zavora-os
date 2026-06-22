use axum::{extract::State, Json};

use crate::greeting::{self, GreetingPayload};
use crate::state::AppState;

pub async fn get_greeting(State(state): State<AppState>) -> Json<GreetingPayload> {
    let payload = greeting::compose(
        state.greeting_runner.as_ref(),
        state.morning_mcp.as_deref(),
        &state.brand_greeting_body,
        state.brand_tone.as_deref(),
    )
    .await;
    Json(payload)
}