//! Tasks API (S4-T3): `GET /api/tasks` — the user's tasks across both worlds for the TODAY view
//! and the approvals UI. Writes go through the agents' gated tools, or later through the
//! command center (S10).

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::domain::Domain;
use crate::routes::actions::resolve_user;
use crate::state::AppState;
use crate::tools::tasks::{TaskFilter, TaskKind, TaskStatus};

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub session_id: Option<String>,
    pub domain: Option<Domain>,
    /// open (default) | done | cancelled | all
    pub status: Option<String>,
    pub kind: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, Response> {
    state.awp.check(&headers, "tasks", "list_tasks").await?;
    let user = resolve_user(&state, &headers, q.session_id.as_deref()).await?;
    let status = match q.status.as_deref() {
        None => Some(TaskStatus::Open),
        Some("all") => None,
        Some(s) => Some(
            TaskStatus::parse(s)
                .ok_or_else(|| (StatusCode::BAD_REQUEST, "status must be open|done|cancelled|all").into_response())?,
        ),
    };
    let kind = match q.kind.as_deref() {
        None => None,
        Some(k) => match TaskKind::parse(k) {
            Some(k) => Some(k),
            None => return Err((StatusCode::BAD_REQUEST, "unknown kind").into_response()),
        },
    };
    let tasks = state
        .tasks
        .list(&user, Domain::Shared, &TaskFilter { domain: q.domain, status, kind, ..Default::default() })
        .await;
    Ok(Json(serde_json::json!({
        "user_id": user,
        "persisted": state.tasks.postgres_enabled(),
        "count": tasks.len(),
        "tasks": tasks,
    })))
}
