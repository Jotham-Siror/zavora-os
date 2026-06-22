use axum::{extract::State, Json};
use serde::Serialize;

use crate::rails::{background, live, people};
use crate::state::AppState;

#[derive(Serialize)]
pub struct BackgroundResponse {
    pub cards: Vec<background::BackgroundCard>,
}

/// Ambient flank cards — intents and rows driven by server state.
pub async fn get_background(State(state): State<AppState>) -> Json<BackgroundResponse> {
    let slack = state.people_mcp.as_ref().and_then(|p| p.slack.clone());
    let people_rail = people::fetch(slack).await;

    let live_slides = if let Some(pool) = state.live_mcp.as_ref() {
        live::fetch_slides(pool.news.clone()).await.1
    } else {
        vec![]
    };

    let proactive = state.ambient.list().await;

    Json(BackgroundResponse {
        cards: background::build(
            &people_rail.work,
            &people_rail.family,
            &live_slides,
            &proactive,
        ),
    })
}