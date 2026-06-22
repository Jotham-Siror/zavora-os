use axum::{extract::State, Json};

use crate::rails::people;
use crate::state::AppState;

/// People rail tiles — Slack users when connected; honest empty state otherwise.
pub async fn get_people(State(state): State<AppState>) -> Json<people::PeopleRail> {
    let slack = state
        .people_mcp
        .as_ref()
        .and_then(|p| p.slack.clone());
    Json(people::fetch(slack).await)
}