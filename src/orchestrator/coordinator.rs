//! Suzy summaries, audio clip matching, and tour suggestions.

use std::sync::Arc;

use adk_runner::Runner;
use tokio::sync::mpsc::Sender;

use crate::events::mock;
use crate::events::sse::{to_event, FieldEvent};
use crate::scenarios::tour;
use crate::state::SessionStore;

fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Match dynamic summary text to a prerecorded audio clip id, if any.
pub fn audio_clip_for(html: &str, scenario: &str) -> Option<String> {
    let plain = strip_html(html);
    for key in crate::agents::router::SCENARIOS {
        let static_plain = strip_html(mock::suzy_summary(key));
        if plain == static_plain {
            return Some((*key).to_string());
        }
    }
    if plain.len() < 12 {
        return Some(scenario.to_string());
    }
    tracing::debug!(
        scenario,
        "no prerecorded audio clip for dynamic summary — queue gen_audio.py if needed"
    );
    None
}

pub async fn suzy_html(
    suzy_runner: Option<&Arc<Runner>>,
    store: &SessionStore,
    session_id: &str,
    user_id: &str,
    scenario: &str,
) -> (String, Option<String>) {
    let fallback = mock::suzy_summary(scenario).to_string();

    let html = if let Some(runner) = suzy_runner {
        if let Some(record) = store.get(session_id).await {
            match crate::agents::suzy::summarize(runner, user_id, session_id, &record).await {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!("Suzy summary fallback ({e:#})");
                    fallback
                }
            }
        } else {
            fallback
        }
    } else {
        fallback
    };

    let audio_clip = audio_clip_for(&html, scenario);
    (html, audio_clip)
}

pub async fn emit_suzy_and_suggest(
    tx: &Sender<Result<axum::response::sse::Event, std::convert::Infallible>>,
    suzy_runner: Option<&Arc<Runner>>,
    store: &SessionStore,
    session_id: &str,
    user_id: &str,
    scenario: &str,
) {
    let (html, audio_clip) =
        suzy_html(suzy_runner, store, session_id, user_id, scenario).await;

    let _ = tx
        .send(Ok(to_event(&FieldEvent::SuzySummary {
            key: scenario.into(),
            html,
            audio_clip,
        })))
        .await;

    if let Some(text) = tour::action_prompt(scenario) {
        let _ = tx
            .send(Ok(to_event(&FieldEvent::Suggest {
                text: text.into(),
                kind: "action".into(),
            })))
            .await;
    }
}

pub async fn emit_tour_advance(
    tx: &Sender<Result<axum::response::sse::Event, std::convert::Infallible>>,
    current: &str,
) {
    if let Some(next) = tour::next_scenario(current) {
        if let Some(text) = tour::scenario_prompt(next) {
            let _ = tx
                .send(Ok(to_event(&FieldEvent::Suggest {
                    text: text.into(),
                    kind: "scenario".into(),
                })))
                .await;
        }
    }
}