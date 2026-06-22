use std::sync::Arc;

use adk_runner::Runner;
use tokio_stream::wrappers::ReceiverStream;

use crate::orchestrator::workflow::{stream_workflow, AgentSlot, WorkflowStreamConfig};
use crate::state::SessionStore;

fn lisbon_cards() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "glyph":"✈️","title":"Flights","agent":"travel.agent",
            "stream":["Scanning carriers for Lisbon…","Comparing price vs. travel time…"]
        }),
        serde_json::json!({
            "glyph":"🏠","title":"Stay","agent":"stay.agent",
            "stream":["Matching neighborhoods…","Filtering for walkable + views…"]
        }),
        serde_json::json!({
            "glyph":"🗺️","title":"Itinerary","agent":"planner.agent","waitsFor":2,
            "stream":["Waiting for flights & stay…","Building day-by-day plan…"]
        }),
    ]
}

fn status_line(tool: &str) -> &'static str {
    match tool {
        "geocode" | "search_poi" | "get_route" => "Building day-by-day plan…",
        "get_forecast" | "geocode_location" => "Checking weather…",
        "geocode_search" | "search_properties_nearby" => "Matching neighborhoods…",
        _ => "Working…",
    }
}

fn flights_resolve(text: &str) -> serde_json::Value {
    serde_json::json!({
        "big": "$284 · TAP Air (stub)",
        "sub": if text.contains("STUB") {
            "Not a live quote — BK-009 travel gap"
        } else {
            "Fri 6:40pm → Sun 9:15pm · 1 stop"
        },
        "actions": ["Hold seat", "Compare"]
    })
}

fn stay_resolve(text: &str) -> serde_json::Value {
    let stub = text.contains("STUB");
    serde_json::json!({
        "big": if stub { "Alfama loft (stub)" } else { "Stay found" },
        "sub": if stub { "Scout only — connect mcp-real-estate" } else { "$96/night · river view" },
        "actions": ["Reserve", "See 6 more"]
    })
}

fn planner_resolve(text: &str) -> serde_json::Value {
    let lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(3)
        .map(|l| {
            if l.starts_with("<b>") {
                l.to_string()
            } else {
                format!("<b>•</b> {l}")
            }
        })
        .collect();
    let lines = if lines.is_empty() {
        vec![
            "<b>Fri</b> — arrive, sunset at Miradouro".into(),
            "<b>Sat</b> — Sintra day trip".into(),
            "<b>Sun</b> — fly home".into(),
        ]
    } else {
        lines
    };
    serde_json::json!({
        "lines": lines,
        "actions": ["Save plan", "Tweak"]
    })
}

fn resolve(author: &str, tool_text: &str, agent_text: &str) -> serde_json::Value {
    let combined = if agent_text.is_empty() {
        tool_text.to_string()
    } else {
        format!("{tool_text}\n{agent_text}")
    };
    match author {
        "flights_agent" => flights_resolve(&combined),
        "stay_agent" => stay_resolve(&combined),
        "planner_agent" => planner_resolve(&combined),
        _ => serde_json::json!({ "big": "Ready", "sub": "Done", "actions": ["Open"] }),
    }
}

pub fn stream_lisbon(
    runner: Arc<Runner>,
    user_id: String,
    session_id: String,
    intent: String,
    has_maps: bool,
    has_real_estate: bool,
    sessions: Option<SessionStore>,
    suzy_runner: Option<Arc<Runner>>,
) -> ReceiverStream<Result<axum::response::sse::Event, std::convert::Infallible>> {
    stream_workflow(
        runner,
        user_id,
        session_id,
        intent,
        WorkflowStreamConfig {
            scenario_key: "lisbon",
            cards: lisbon_cards(),
            slots: vec![
                AgentSlot {
                    author: "flights_agent",
                    index: 0,
                },
                AgentSlot {
                    author: "stay_agent",
                    index: 1,
                },
                AgentSlot {
                    author: "planner_agent",
                    index: 2,
                },
            ],
            status_line,
            resolve,
            integrations_note: format!(
                "flights: stub (BK-009)\nstay: {}\nmaps: {}\nweather: live",
                if has_real_estate { "live scout" } else { "stub" },
                if has_maps { "live" } else { "weather-only itinerary" },
            ),
        },
        sessions,
        suzy_runner,
    )
}