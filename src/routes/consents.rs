//! Consents API (S11-T4, pulled forward to team sprint A): what the user has allowed, per
//! category and world. `GET` lists grants (including revoked, for history); `PUT` grants or
//! revokes one category. The trust center (S11) and onboarding (S12) build on these two calls.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::domain::Domain;
use crate::memory::consent::{normalize_category, CATEGORIES};
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
    state.awp.check(&headers, "consents", "manage_consents").await?;
    let user = resolve_user(&state, &headers, q.session_id.as_deref()).await?;
    let consents = state.consents.list(&user).await;
    Ok(Json(serde_json::json!({
        "user_id": user,
        "categories": CATEGORIES,
        "persisted": state.consents.postgres_enabled(),
        "consents": consents,
    })))
}

#[derive(Deserialize)]
pub struct PutBody {
    pub session_id: Option<String>,
    pub category: String,
    /// Default `shared` (covers both worlds).
    pub world: Option<Domain>,
    /// Required when granting: the plain-language purpose shown in the trust center.
    pub purpose: Option<String>,
    pub granted: bool,
}

pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<PutBody>,
) -> Result<Json<serde_json::Value>, Response> {
    state.awp.check(&headers, "consents", "manage_consents").await?;
    let user = resolve_user(&state, &headers, body.session_id.as_deref()).await?;
    let Some(category) = normalize_category(&body.category) else {
        return Err((StatusCode::BAD_REQUEST, "category must be lowercase letters or underscores (≤40 chars)").into_response());
    };
    if !CATEGORIES.contains(&category.as_str()) {
        return Err((StatusCode::BAD_REQUEST, format!("unknown category '{category}'; expected one of {CATEGORIES:?}")).into_response());
    }
    if body.granted {
        let purpose = body
            .purpose
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "purpose required when granting").into_response())?;
        state
            .consents
            .grant(&user, &category, body.world.unwrap_or_default(), purpose)
            .await;
    } else {
        state.consents.revoke(&user, &category, body.world).await;
    }
    Ok(Json(serde_json::json!({
        "user_id": user,
        "consents": state.consents.list(&user).await,
    })))
}
