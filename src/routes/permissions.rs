//! Permissions API (S2-T8): per-agent modes, per-tool overrides, pause / resume.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::permissions::{Mode, PauseScope};
use crate::routes::actions::resolve_user;
use crate::state::AppState;

#[derive(Deserialize, Default)]
pub struct SessionQuery {
    pub session_id: Option<String>,
}

pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SessionQuery>,
) -> Result<Json<serde_json::Value>, Response> {
    state.awp.check(&headers, "permissions", "get_permissions").await?;
    let user = resolve_user(&state, &headers, q.session_id.as_deref()).await?;
    let agents = state.permissions.list(&user).await;
    let pause = state.permissions.pause_state().await;
    Ok(Json(serde_json::json!({ "user_id": user, "agents": agents, "pause": pause })))
}

#[derive(Deserialize)]
pub struct PutBody {
    pub session_id: Option<String>,
    pub agent_id: String,
    /// New agent mode (optional when only a tool override changes).
    pub mode: Option<Mode>,
    /// Tool name for a per-tool override…
    pub tool: Option<String>,
    /// …and its mode; `null` clears the override.
    pub tool_mode: Option<Mode>,
}

pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PutBody>,
) -> Result<Json<serde_json::Value>, Response> {
    state.awp.check(&headers, "permissions", "set_permissions").await?;
    let user = resolve_user(&state, &headers, body.session_id.as_deref()).await?;
    if crate::tools::allowlist::catalog().spec_for(&body.agent_id).is_none() {
        return Err((StatusCode::NOT_FOUND, "unknown agent").into_response());
    }
    if let Some(mode) = body.mode {
        state.permissions.set_mode(&user, &body.agent_id, mode).await;
    }
    if let Some(tool) = body.tool.as_deref() {
        state.permissions.set_tool_override(&user, &body.agent_id, tool, body.tool_mode).await;
    }
    let agents = state.permissions.list(&user).await;
    let view = agents.into_iter().find(|a| a.agent_id == body.agent_id);
    Ok(Json(serde_json::json!({ "user_id": user, "agent": view })))
}

#[derive(Deserialize)]
pub struct PauseBody {
    pub session_id: Option<String>,
    #[serde(default = "default_scope")]
    pub scope: PauseScope,
    /// Minutes until automatic resume; omit for indefinite.
    pub minutes: Option<i64>,
}

fn default_scope() -> PauseScope {
    PauseScope::All
}

/// `POST /api/pause` — every write for the scope waits; pausing everything also sets DND.
pub async fn pause(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PauseBody>,
) -> Result<Json<serde_json::Value>, Response> {
    state.awp.check(&headers, "pause", "pause_agents").await?;
    let _user = resolve_user(&state, &headers, body.session_id.as_deref()).await?;
    let until = body.minutes.map(|m| chrono::Utc::now() + chrono::Duration::minutes(m));
    state.permissions.pause(body.scope, until).await;
    if body.scope == PauseScope::All {
        state.ambient.set_dnd(true).await;
    }
    Ok(Json(serde_json::json!({ "pause": state.permissions.pause_state().await, "dnd": state.ambient.dnd().await })))
}

#[derive(Deserialize, Default)]
pub struct ResumeBody {
    pub session_id: Option<String>,
    pub scope: Option<PauseScope>,
}

/// `POST /api/resume` — clear one scope or everything (and DND).
pub async fn resume(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<ResumeBody>>,
) -> Result<Json<serde_json::Value>, Response> {
    let body = body.map(|b| b.0).unwrap_or_default();
    state.awp.check(&headers, "pause", "pause_agents").await?;
    let _user = resolve_user(&state, &headers, body.session_id.as_deref()).await?;
    match body.scope {
        Some(scope) => state.permissions.resume(scope).await,
        None => {
            state.permissions.resume_all().await;
            state.ambient.set_dnd(false).await;
        }
    }
    Ok(Json(serde_json::json!({ "pause": state.permissions.pause_state().await, "dnd": state.ambient.dnd().await })))
}
