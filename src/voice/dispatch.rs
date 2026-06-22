//! Background intent orchestration from voice tool calls.

use axum::body::Body;
use http_body_util::BodyExt as _;

use crate::orchestrator::dispatch::{dispatch_intent, IntentDispatch};
use crate::state::AppState;

/// Run full intent SSE pipeline in the background so session/cards update while voice continues.
pub fn spawn_voice_intent(state: AppState, session_id: String, user_id: String, text: String) {
    tokio::spawn(async move {
        let response = dispatch_intent(IntentDispatch {
            state: &state,
            session_id,
            user_id,
            text,
        })
        .await;

        let body: Body = response.into_body();
        let _ = body.collect().await;
    });
}