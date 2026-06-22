use std::convert::Infallible;

use axum::response::sse::Sse;
use axum::response::{IntoResponse, Response};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::agents::router::{self, ClassifyOutcome};
use crate::events::mock;
use crate::events::sse::{to_event, FieldEvent};
use crate::orchestrator::{combine, deck, morning};
use crate::scenarios;
use crate::scenarios::tour;
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

async fn resolve_scenario(state: &AppState, user_id: &str, session_id: &str, text: &str) -> String {
    if let Some(runner) = state.router_runner.as_ref() {
        match router::classify(runner, user_id, session_id, text, mock::pick_scenario).await {
            ClassifyOutcome::Scenario(key) => return key,
            ClassifyOutcome::Clarify(_) => return "clarify".into(),
        }
    }
    mock::pick_scenario(text).into()
}

fn stream_clarify(message: String) -> Response {
    let (tx, rx) = mpsc::channel::<Result<axum::response::sse::Event, Infallible>>(8);
    tokio::spawn(async move {
        let _ = tx
            .send(Ok(to_event(&FieldEvent::SuzySummary {
                key: "clarify".into(),
                html: message,
                audio_clip: None,
            })))
            .await;
        for key in ["morning", "deck", "lisbon"] {
            if let Some(text) = tour::scenario_prompt(key) {
                let _ = tx
                    .send(Ok(to_event(&FieldEvent::Suggest {
                        text: text.into(),
                        kind: "scenario".into(),
                    })))
                    .await;
            }
        }
        let _ = tx.send(Ok(to_event(&FieldEvent::Done))).await;
    });
    Sse::new(ReceiverStream::new(rx)).into_response()
}

pub async fn dispatch_intent(req: IntentDispatch<'_>) -> Response {
    if let Some(runner) = req.state.router_runner.as_ref() {
            if let ClassifyOutcome::Clarify(msg) =
                router::classify(runner, &req.user_id, &req.session_id, &req.text, mock::pick_scenario)
                    .await
            {
                return stream_clarify(msg);
            }
    }

    let scenario = resolve_scenario(req.state, &req.user_id, &req.session_id, &req.text).await;

    if scenario == "clarify" {
        return stream_clarify(
            "I'm not sure which flow you want. Try <b>Start my day</b>, <b>Build me a pitch deck</b>, or <b>Plan a trip to Lisbon</b>.".into(),
        );
    }

    if scenarios::intent_is_live(
        &scenario,
        req.state.deck_enabled,
        req.state.morning_enabled,
    ) {
        match scenario.as_str() {
            "deck" => {
                if let Some(runner) = req.state.deck_runner.clone() {
                    return Sse::new(deck::stream_deck(
                        runner,
                        req.user_id,
                        req.session_id,
                        req.text,
                        req.state.artifact_dir.clone(),
                        Some(req.state.sessions.clone()),
                        req.state.suzy_runner.clone(),
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
                        req.state.suzy_runner.clone(),
                    ))
                    .into_response();
                }
            }
            _ => {}
        }
    }

    Sse::new(mock::stream_intent_with_scenario(
        &scenario,
        &req.text,
        if req.state.coordinator_enabled {
            req.state.suzy_runner.clone()
        } else {
            None
        },
        Some(req.state.sessions.clone()),
        Some(req.session_id),
        Some(req.user_id),
    ))
    .into_response()
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
                    req.scenario.clone(),
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
pub type SseStream = ReceiverStream<Result<axum::response::sse::Event, Infallible>>;