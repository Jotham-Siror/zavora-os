use axum::{extract::State, Json};
use serde_json::json;

use crate::state::AppState;

/// People rail tiles — Slack users when connected, static demo fallback.
pub async fn get_people(State(state): State<AppState>) -> Json<serde_json::Value> {
    let source = if state
        .people_mcp
        .as_ref()
        .is_some_and(|p| p.slack.is_some())
    {
        "slack"
    } else {
        "demo"
    };

    Json(json!({
        "source": source,
        "work": [
            ["AL", "Alex Mboya", if source == "slack" { "active on Slack" } else { "typing…" }, true],
            ["PR", "Priya N.", if source == "slack" { "in a meeting" } else { "in a meeting" }, false],
            ["DEV", "#dev-team", if source == "slack" { "live channel" } else { "3 new" }, true]
        ],
        "family": [
            ["MA", "Mara", "💚 home", true],
            ["DAD", "Dad", "called 2×", false]
        ]
    }))
}