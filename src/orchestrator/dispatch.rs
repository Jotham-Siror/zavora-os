use axum::response::sse::Sse;
use axum::response::{IntoResponse, Response};
use tokio_stream::wrappers::ReceiverStream;

use crate::events::mock;
use crate::orchestrator::{combine, deck, morning};
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

    if scenarios::intent_is_live(
        scenario,
        req.state.deck_enabled,
        req.state.morning_enabled,
    ) {
        match scenario {
            "deck" => {
                if let Some(runner) = req.state.deck_runner.clone() {
                    return Sse::new(deck::stream_deck(
                        runner,
                        req.user_id,
                        req.session_id,
                        req.text,
                        req.state.artifact_dir.clone(),
                        Some(req.state.sessions.clone()),
                    ))
                    .into_response();
                }
            }
            "morning" => {
                if let Some(runner) = req.state.morning_runner.clone() {
                    let has_calendar = req
                        .state
                        .morning_mcp
                        .as_ref()
                        .is_some_and(|p| p.calendar.is_some());
                    let has_inbox = req
                        .state
                        .morning_mcp
                        .as_ref()
                        .is_some_and(|p| p.email.is_some());
                    return Sse::new(morning::stream_morning(
                        runner,
                        req.user_id,
                        req.session_id,
                        req.text,
                        Some(req.state.sessions.clone()),
                        has_calendar,
                        has_inbox,
                    ))
                    .into_response();
                }
            }
            _ => {}
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