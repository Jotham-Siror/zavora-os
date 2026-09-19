//! Delegation planner and merged streaming (S1-T3).
//!
//! `handle_intent` is the Mother Agent's runtime entry: intake → (clarify | single target
//! pass-through | multi-target fan-out) → synthesis → SSE. Targets are fulfilled by the Phase 1
//! scenario workflows through `crate::orchestrator::dispatch::stream_scenario` until the world
//! agents ship (S4/S5). Multi-target fan-out runs every target concurrently, re-indexes their
//! cards into one field, persists the merged cards, and closes with one Suzy synthesis.

use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::Sse;
use axum::response::{IntoResponse, Response};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::domain::Domain;
use crate::events::sse::{to_event, FieldEvent};
use crate::mother::intake::{self, IntakeResult, Target};
use crate::mother::synth::{self, CardOutcome, TargetResult};
use crate::orchestrator::{dispatch, persist, sse_collect};
use crate::scenarios::tour;
use crate::state::AppState;

/// Where the utterance came from. Recorded on chat turns and (from S2) in the ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Intent,
    Chat,
    Voice,
    A2a,
}

impl Entry {
    pub fn as_str(&self) -> &'static str {
        match self {
            Entry::Intent => "intent",
            Entry::Chat => "chat",
            Entry::Voice => "voice",
            Entry::A2a => "a2a",
        }
    }
}

pub struct MotherRequest<'a> {
    pub state: &'a AppState,
    pub session_id: String,
    pub user_id: String,
    pub text: String,
    pub entry: Entry,
}

/// Per-target fan-out budget. Mock scenarios finish in a few seconds; live MCP workflows can
/// take longer, so the budget is generous and a timed-out target still contributes its
/// partial cards.
pub const TARGET_TIMEOUT: Duration = Duration::from_secs(90);

/// The Mother Agent's runtime pipeline for one utterance.
pub async fn handle_intent(req: MotherRequest<'_>) -> Response {
    let MotherRequest { state, session_id, user_id, text, entry } = req;

    if matches!(entry, Entry::Chat) {
        state.sessions.append_chat(&session_id, "user", &text).await;
    }

    let intake = intake::classify(
        state.router_runner.as_deref(),
        &user_id,
        &session_id,
        &text,
    )
    .await;
    tracing::info!(
        entry = entry.as_str(),
        source = intake.source,
        targets = intake.targets.len(),
        domains = ?intake.domains,
        "mother intake"
    );

    if let Some(msg) = intake.clarify.clone() {
        if matches!(entry, Entry::Chat) {
            state.sessions.append_chat(&session_id, "mother", &synth::strip_tags(&msg)).await;
        }
        return dispatch::stream_clarify(msg);
    }

    if !intake.is_multi_target() {
        let scenario = intake
            .primary_scenario()
            .unwrap_or_else(|| crate::events::mock::pick_scenario(&text))
            .to_string();
        return dispatch::stream_scenario(state, &scenario, session_id, user_id, text, true).await;
    }

    stream_multi_target(state, session_id, user_id, text, intake)
}

/// Fan out to every target concurrently, merge, persist, synthesize, stream.
fn stream_multi_target(
    state: &AppState,
    session_id: String,
    user_id: String,
    text: String,
    intake: IntakeResult,
) -> Response {
    let (tx, rx) = mpsc::channel::<Result<axum::response::sse::Event, Infallible>>(128);
    let state = state.clone();

    tokio::spawn(async move {
        let targets = intake.targets.clone();
        let primary = intake.primary_scenario().unwrap_or("morning").to_string();

        let mut collected: Vec<(Vec<serde_json::Value>, bool)> = Vec::with_capacity(targets.len());
        let futures: Vec<_> = targets
            .iter()
            .map(|t| {
                let state = state.clone();
                let (sid, uid, txt, scenario) = (session_id.clone(), user_id.clone(), text.clone(), t.scenario.clone());
                async move {
                    let resp = dispatch::stream_scenario(&state, &scenario, sid, uid, txt, false).await;
                    match tokio::time::timeout(TARGET_TIMEOUT, sse_collect::collect_sse_events(resp)).await {
                        Ok(events) => (events, false),
                        Err(_) => (Vec::new(), true),
                    }
                }
            })
            .collect();
        for out in futures::future::join_all(futures).await {
            collected.push(out);
        }

        let merged = merge_target_events(&targets, collected);

        // Persist the merged field under the primary scenario key.
        state.sessions.set_scenario(&session_id, &primary, Some(&text)).await;
        for ev in &merged.events {
            match ev {
                FieldEvent::CardSpawn { index, card, .. } => {
                    persist::card_spawn(&state.sessions, &session_id, *index, card.clone()).await;
                }
                FieldEvent::CardResolve { index, resolve } => {
                    let card = merged.card_at(*index).cloned().unwrap_or_default();
                    persist::card_resolve(&state.sessions, &session_id, *index, card, resolve.clone(), false).await;
                }
                _ => {}
            }
        }

        let _ = tx
            .send(Ok(to_event(&FieldEvent::Scenario {
                key: primary.clone(),
                text: text.clone(),
                total_cards: merged.total_cards,
            })))
            .await;
        for ev in &merged.events {
            let _ = tx.send(Ok(to_event(ev))).await;
        }

        let mode_for = |agent: &str| crate::tools::allowlist::catalog().mode_for(&agent_id_from_card_agent(agent));
        let synthesis = synth::compose(
            state.suzy_runner.as_ref().filter(|_| state.coordinator_enabled),
            &user_id,
            &session_id,
            &text,
            &merged.results,
            &[],
            &mode_for,
        )
        .await;

        state
            .sessions
            .append_chat(&session_id, "mother", &synth::strip_tags(&synthesis.html))
            .await;

        let _ = tx
            .send(Ok(to_event(&FieldEvent::SuzySummary {
                key: "mother".into(),
                html: synthesis.html.clone(),
                audio_clip: None,
            })))
            .await;
        for a in synthesis.actions.iter().take(4) {
            let _ = tx
                .send(Ok(to_event(&FieldEvent::Suggest {
                    text: format!("{} {}", a.mode.badge(), a.text),
                    kind: "action".into(),
                })))
                .await;
        }
        if let Some(next) = tour::next_scenario(&primary).and_then(tour::scenario_prompt) {
            let _ = tx
                .send(Ok(to_event(&FieldEvent::Suggest {
                    text: next.into(),
                    kind: "scenario".into(),
                })))
                .await;
        }
        let _ = tx.send(Ok(to_event(&FieldEvent::Done))).await;
    });

    Sse::new(ReceiverStream::new(rx)).into_response()
}

/// Map a card's `agent` label (`inbox.agent`, `auto-excel`, …) to the allowlist agent id
/// (`inbox_agent`, `excel_agent`, …) so modes can be looked up. Unknown → the label itself.
pub fn agent_id_from_card_agent(label: &str) -> String {
    match label {
        "calendar.agent" => "calendar_agent",
        "inbox.agent" => "inbox_agent",
        "news.agent" => "brief_agent",
        "auto-excel" => "excel_agent",
        "auto-docs" => "docs_agent",
        "auto-slides" => "slides_agent",
        "team.agent" => "team_agent",
        "people.agent" => "priya_agent",
        "crm.agent" => "connections_agent",
        "finance.agent" => "money_agent",
        "health.agent" => "health_agent",
        "work.agent" => "focus_agent",
        "travel.agent" => "flights_agent",
        "stay.agent" => "stay_agent",
        "planner.agent" => "planner_agent",
        "markets.agent" => "markets_agent",
        "live.agent" => "now_agent",
        "research.agent" => "research_agent",
        "scout.agent" => "scout_agent",
        "maker.agent" => "maker_agent",
        other => other,
    }
    .to_string()
}

/// Result of merging several targets' SSE event lists into one card field.
#[derive(Debug, Default)]
pub struct Merged {
    pub total_cards: usize,
    pub events: Vec<FieldEvent>,
    pub results: Vec<TargetResult>,
    cards: Vec<(usize, serde_json::Value)>,
}

impl Merged {
    pub fn card_at(&self, index: usize) -> Option<&serde_json::Value> {
        self.cards.iter().find(|(i, _)| *i == index).map(|(_, c)| c)
    }
}

fn idx(v: &serde_json::Value) -> Option<usize> {
    v.get("index").and_then(|i| i.as_u64()).map(|i| i as usize)
}

/// Pure merge: re-index each target's `card_*` events by a cumulative offset, drop each
/// target's own `scenario` / `suzy_summary` / `suggest` / `done` (the Mother emits one of each),
/// and build the per-target [`TargetResult`]s for synthesis.
pub fn merge_target_events(targets: &[Target], per_target: Vec<(Vec<serde_json::Value>, bool)>) -> Merged {
    let mut merged = Merged::default();
    let mut offset = 0usize;

    for (target, (events, timed_out)) in targets.iter().zip(per_target.into_iter()) {
        let declared = events
            .iter()
            .find(|e| e.get("type").and_then(|t| t.as_str()) == Some("scenario"))
            .and_then(|e| e.get("total_cards").and_then(|n| n.as_u64()))
            .map(|n| n as usize);
        let max_index = events.iter().filter_map(idx).max().map(|m| m + 1).unwrap_or(0);
        let count = declared.unwrap_or(max_index).max(max_index);

        let mut cards: Vec<CardOutcome> = Vec::new();
        for e in &events {
            let Some(kind) = e.get("type").and_then(|t| t.as_str()) else { continue };
            match kind {
                "card_spawn" => {
                    let Some(i) = idx(e) else { continue };
                    let card = e.get("card").cloned().unwrap_or_default();
                    // Precedence: the card's own explicit `domain` → the delegating target's
                    // world (it knows why it asked) → the streamed/derived domain.
                    let explicit = card.get("domain").and_then(|d| d.as_str()).and_then(Domain::parse);
                    let streamed = e
                        .get("domain")
                        .and_then(|d| d.as_str())
                        .and_then(Domain::parse)
                        .unwrap_or_else(|| Domain::for_card(&target.scenario, &card));
                    let domain = match (explicit, target.world) {
                        (Some(d), _) => d,
                        (None, Domain::Shared) => streamed,
                        (None, world) => world,
                    };
                    let card = {
                        let mut c = card;
                        if let Some(obj) = c.as_object_mut() {
                            obj.entry("domain").or_insert(serde_json::json!(domain.as_str()));
                        }
                        c
                    };
                    cards.push(CardOutcome {
                        title: card.get("title").and_then(|t| t.as_str()).unwrap_or("card").to_string(),
                        agent: card.get("agent").and_then(|a| a.as_str()).unwrap_or("").to_string(),
                        domain,
                        resolve: None,
                    });
                    merged.cards.push((offset + i, card.clone()));
                    merged.events.push(FieldEvent::CardSpawn { index: offset + i, card, domain });
                }
                "card_status" => {
                    let Some(i) = idx(e) else { continue };
                    merged.events.push(FieldEvent::CardStatus {
                        index: offset + i,
                        status: e.get("status").and_then(|s| s.as_str()).unwrap_or("working").to_string(),
                        line: e.get("line").and_then(|l| l.as_str()).map(str::to_string),
                    });
                }
                "card_surface" => {
                    let Some(i) = idx(e) else { continue };
                    merged.events.push(FieldEvent::CardSurface {
                        index: offset + i,
                        surface: e.get("surface").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                        slide: e.get("slide").and_then(|s| s.as_u64()).unwrap_or(0) as u32,
                        total: e.get("total").and_then(|s| s.as_u64()).unwrap_or(0) as u32,
                    });
                }
                "card_resolve" => {
                    let Some(i) = idx(e) else { continue };
                    let resolve = e.get("resolve").cloned().unwrap_or_default();
                    if let Some(c) = cards.get_mut(i) {
                        c.resolve = Some(resolve.clone());
                    }
                    merged.events.push(FieldEvent::CardResolve { index: offset + i, resolve });
                }
                "error" => {
                    merged.events.push(FieldEvent::Error {
                        message: e.get("message").and_then(|m| m.as_str()).unwrap_or("error").to_string(),
                    });
                }
                _ => {}
            }
        }

        merged.results.push(TargetResult { target: target.clone(), cards, timed_out });
        offset += count;
    }

    merged.total_cards = offset;
    merged
}
