use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct PublicConfig {
    pub signup_endpoint: Option<String>,
    pub linkedin_partner_id: Option<String>,
    pub linkedin_conversion_id: Option<u64>,
    pub allow_demo_mode: bool,
    pub public_domain: String,
    pub uses_mock_orchestration: bool,
}

pub async fn public_config(State(state): State<AppState>) -> Json<PublicConfig> {
    let r = &state.runtime;
    Json(PublicConfig {
        signup_endpoint: r.signup_endpoint.clone(),
        linkedin_partner_id: r.linkedin_partner_id.clone(),
        linkedin_conversion_id: r.linkedin_conversion_id,
        allow_demo_mode: r.allow_demo_mode,
        public_domain: r.public_domain.clone(),
        uses_mock_orchestration: r.uses_mock_orchestration,
    })
}