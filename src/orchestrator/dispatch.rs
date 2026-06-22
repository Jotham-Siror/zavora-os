use axum::response::sse::Sse;
use axum::response::{IntoResponse, Response};
use tokio_stream::wrappers::ReceiverStream;

use crate::events::mock;
use crate::orchestrator::{combine, deck};
use crate::scenarios;
use crate::state::AppState;

pub struct IntentDispatch<'a> {
    pub state: &'a AppState,
    pub session_id: String,
    pub user_id: String,
    pub text: String,
}

pub struct ActionDispatch<'a> {
    pub state: &'a AppState,
    pub session_id: String,
    pub user_id: String,
    pub text: String,
    pub action: &'static str,
    pub scenario: Option<String>,
}

pub fn dispatch_intent(req: IntentDispatch<'_>) -> Response {
    let scenario = scenarios::pick_scenario(&req.text);

    if scenarios::intent_is_live(scenario, req.state.deck_enabled) {
        if scenario == "deck" {
            if let Some(runner) = req.state.deck_runner.clone() {
                let session_id = req.session_id.clone();
                let user_id = req.user_id.clone();
                let text = req.text.clone();
                let artifact_dir = req.state.artifact_dir.clone();
                let sessions = req.state.sessions.clone();

                return Sse::new(deck::stream_deck(
                    runner,
                    user_id,
                    session_id.clone(),
                    text,
                    artifact_dir,
                    Some(sessions),
                ))
                .into_response();
            }
        }
    }

    Sse::new(mock::stream_intent(&req.text)).into_response()
}

pub fn dispatch_action(req: ActionDispatch<'_>) -> Response {
    if scenarios::action_is_live(req.action, req.scenario.as_deref(), req.state.deck_enabled) {
        if req.action == "combine" {
            if let Some(runner) = req.state.combine_runner.clone() {
                return Sse::new(combine::stream_combine(
                    runner,
                    req.user_id,
                    req.session_id,
                    req.text,
                    req.state.artifact_dir.clone(),
                    req.state.sessions.clone(),
                ))
                .into_response();
            }
        }
    }

    Sse::new(mock::stream_action(
        req.action,
        req.scenario.as_deref(),
        &req.text,
    ))
    .into_response()
}

/// Type alias for SSE stream returned by orchestrators.
pub type SseStream = ReceiverStream<Result<axum::response::sse::Event, std::convert::Infallible>>;