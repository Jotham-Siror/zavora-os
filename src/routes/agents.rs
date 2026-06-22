use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::{AgentRecord, AppState};

#[derive(Serialize)]
pub struct AgentsResponse {
    pub active: Vec<AgentRecord>,
    pub resting: Vec<AgentRecord>,
}

#[derive(Deserialize)]
pub struct AgentBody {
    pub session_id: String,
    pub title: String,
    pub glyph: Option<String>,
    pub agent: Option<String>,
}

pub async fn list_agents(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<AgentsResponse>, StatusCode> {
    let Some((active, resting)) = state.sessions.list_agents(&session_id).await else {
        return Err(StatusCode::NOT_FOUND);
    };
    Ok(Json(AgentsResponse { active, resting }))
}

pub async fn snooze_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(body): Json<AgentBody>,
) -> Result<StatusCode, StatusCode> {
    if !state.sessions.get(&body.session_id).await.is_some() {
        return Err(StatusCode::NOT_FOUND);
    }
    let glyph = body.glyph.unwrap_or_else(|| "💤".into());
    let agent_name = body.agent.unwrap_or_else(|| agent_id.clone());
    state
        .sessions
        .agent_snooze(
            &body.session_id,
            AgentRecord {
                id: agent_id,
                title: body.title.clone(),
                glyph,
                agent: agent_name,
                rail: "resting".into(),
            },
        )
        .await;
    state
        .sessions
        .remove_card(&body.session_id, &body.title)
        .await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn wake_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(body): Json<AgentBody>,
) -> Result<Json<AgentRecord>, StatusCode> {
    let Some(agent) = state.sessions.agent_wake(&body.session_id, &agent_id).await else {
        return Err(StatusCode::NOT_FOUND);
    };
    Ok(Json(agent))
}