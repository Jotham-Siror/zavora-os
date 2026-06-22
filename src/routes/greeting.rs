use axum::{extract::State, Json};
use serde::Serialize;

use crate::greeting;
use crate::state::AppState;

#[derive(Serialize)]
pub struct GreetingResponse {
    pub text: String,
    pub source: &'static str,
    pub audio_clip: &'static str,
}

pub async fn get_greeting(State(state): State<AppState>) -> Json<GreetingResponse> {
    let (text, source) = greeting::personalized_text(state.morning_mcp.as_deref()).await;
    Json(GreetingResponse {
        text,
        source,
        audio_clip: "/audio/greeting.wav",
    })
}