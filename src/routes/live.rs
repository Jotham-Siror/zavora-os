use axum::{extract::State, Json};
use serde_json::json;

use crate::state::AppState;

/// Live carousel slides — mcp-news when live stack enabled, demo fallback.
pub async fn get_live(State(state): State<AppState>) -> Json<serde_json::Value> {
    let source = if state.scenario_flags.live { "mcp-news" } else { "demo" };

    Json(json!({
        "source": source,
        "slides": [
            {
                "cls": "lf-cnn",
                "logo": "C",
                "name": "Headlines",
                "when": "now",
                "body": if source == "mcp-news" {
                    "Live headlines from mcp-news — open a Live scenario for full cards"
                } else {
                    "Breaking: central banks signal a pause on rate hikes"
                },
                "meta": if source == "mcp-news" { "mcp-news" } else { "World · 2m read" }
            },
            {
                "cls": "lf-bloomberg",
                "logo": "B",
                "name": "Markets",
                "when": "4m",
                "body": "Markets edge up ahead of the open; chips lead gains",
                "meta": "Markets"
            }
        ]
    }))
}