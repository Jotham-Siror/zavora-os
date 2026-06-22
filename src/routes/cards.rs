use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::state::{AppState, CardRecord};

#[derive(Serialize)]
pub struct CardsResponse {
    pub session_id: String,
    pub scenario: Option<String>,
    pub origin_text: Option<String>,
    pub cards: Vec<CardRecord>,
}

pub async fn list_cards(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<CardsResponse>, StatusCode> {
    let Some(record) = state.sessions.get(&session_id).await else {
        return Err(StatusCode::NOT_FOUND);
    };
    let cards = record
        .cards
        .into_iter()
        .filter(|c| !c.removed)
        .collect();
    Ok(Json(CardsResponse {
        session_id: record.session_id,
        scenario: record.scenario,
        origin_text: record.origin_text,
        cards,
    }))
}