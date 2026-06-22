use axum::{extract::State, Json};
use serde::Serialize;

use crate::rails::live;
use crate::state::AppState;

#[derive(Serialize)]
pub struct LiveResponse {
    pub source: String,
    pub slides: Vec<live::LiveSlide>,
}

/// Live carousel — real headlines from mcp-news when the live stack is enabled.
pub async fn get_live(State(state): State<AppState>) -> Json<LiveResponse> {
    if let Some(pool) = state.live_mcp.as_ref() {
        let (source, slides) = live::fetch_slides(pool.news.clone()).await;
        return Json(LiveResponse { source, slides });
    }

    Json(LiveResponse {
        source: "unavailable".into(),
        slides: vec![],
    })
}