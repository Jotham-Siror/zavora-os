//! Work World and Home World coordinating agents (ADR-002; concept §4–§5).
//!
//! [`work`] ships in S4 (`work_mother`, work agent registry, follow-up tracker). `home` lands in S5.
//! [`fan_out`] is what the Mother calls for a multi-target turn: every target runs concurrently;
//! targets that belong to a world with a mother are folded into that world's structured result
//! and the world may append outcomes of its own (follow-ups, labeled stubs).

pub mod work;

use std::time::Duration;

use crate::domain::Domain;
use crate::mother::intake::Target;
use crate::orchestrator::{dispatch, sse_collect};
use crate::state::AppState;

/// Per-target fan-out budget (mock scenarios finish in seconds; live MCP workflows take longer).
pub const TARGET_TIMEOUT: Duration = Duration::from_secs(90);

/// Run one target's scenario without persisting and collect its events.
pub async fn run_target(state: &AppState, session_id: &str, user_id: &str, text: &str, target: &Target) -> (Vec<serde_json::Value>, bool) {
    if target.scenario == "stub" {
        return (Vec::new(), false);
    }
    let resp = dispatch::stream_scenario(state, &target.scenario, session_id.to_string(), user_id.to_string(), text.to_string(), false).await;
    match tokio::time::timeout(TARGET_TIMEOUT, sse_collect::collect_sse_events(resp)).await {
        Ok(events) => (events, false),
        Err(_) => (Vec::new(), true),
    }
}

/// What a fan-out produced, ready for `mother::delegate::merge_target_events`.
pub struct FanOut {
    pub targets: Vec<Target>,
    pub collected: Vec<(Vec<serde_json::Value>, bool)>,
    pub work: Option<work::WorldResult>,
}

/// Fan out all targets concurrently, then let each world fold its share into one structured
/// result and append its own outcomes.
pub async fn fan_out(state: &AppState, session_id: &str, user_id: &str, text: &str, targets: Vec<Target>) -> FanOut {
    let futures: Vec<_> = targets.iter().map(|t| run_target(state, session_id, user_id, text, t)).collect();
    let mut collected: Vec<(Vec<serde_json::Value>, bool)> = futures::future::join_all(futures).await;
    let mut targets = targets;
    let mut work_result = None;

    if work::enabled() && targets.iter().any(|t| t.world == Domain::Work) {
        let (result, extra) = work::fold(&state.ledger, user_id, &targets, &collected).await;
        for (t, events) in extra {
            targets.push(t);
            collected.push((events, false));
        }
        work_result = Some(result);
    }

    FanOut { targets, collected, work: work_result }
}
