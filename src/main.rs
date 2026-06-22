mod config;
mod events;
mod routes;
mod state;

use std::path::Path;
use std::sync::Arc;

use adk_awp::{
    handlers, middleware::version_negotiation, AwpState, BusinessContextLoader,
    DefaultTrustAssigner, HealthStateMachine, InMemoryConsentService,
    InMemoryEventSubscriptionService, InMemoryRateLimiter,
};
use axum::middleware::from_fn;
use axum::routing::{delete, get, post};
use axum::{Extension, Router};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use config::AppConfig;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = AppConfig::from_env()?;
    let app_state = AppState::new();
    let session_store = app_state.sessions.clone();

    let loader = BusinessContextLoader::from_file(&config.business_toml)?;
    let ctx = loader.load();
    tracing::info!("Loaded business context: {}", ctx.site_name);

    let event_service = Arc::new(InMemoryEventSubscriptionService::new());
    let awp_state = AwpState {
        business_context: loader.context_ref(),
        rate_limiter: Arc::new(InMemoryRateLimiter::new()),
        consent_service: Arc::new(InMemoryConsentService::new()),
        event_service: event_service.clone(),
        health: Arc::new(HealthStateMachine::new(event_service)),
        trust_assigner: Arc::new(DefaultTrustAssigner),
    };

    let api = Router::new()
        .route("/health", get(routes::health::health))
        .route("/api/sessions", post(routes::session::create_session))
        .route(
            "/api/sessions/{session_id}/intent",
            post(routes::intent::submit_intent),
        )
        .with_state(app_state)
        .merge(awp_router(awp_state))
        .layer(CorsLayer::very_permissive());

    let mut app = api.layer(Extension(session_store));

    if Path::new(&config.audio_dir).exists() {
        app = app.nest_service("/audio", ServeDir::new(&config.audio_dir));
    }

    if Path::new(&config.static_dir).exists() {
        app = app.nest_service("/static", ServeDir::new(&config.static_dir));
    }

    if Path::new(&config.web_dir).exists() {
        app = app.fallback_service(
            ServeDir::new(&config.web_dir).append_index_html_on_directories(true),
        );
    } else {
        tracing::warn!("web dir missing at {}", config.web_dir.display());
    }

    let addr = config.addr();
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Zavora OS listening on http://{addr}");

    axum::serve(listener, app).await?;
    Ok(())
}

/// AWP routes with Zavora-specific `/awp/a2a` → intent session bootstrap.
fn awp_router(state: AwpState) -> Router {
    Router::new()
        .route("/.well-known/awp.json", get(handlers::discovery))
        .route("/awp/manifest", get(handlers::manifest))
        .route("/awp/health", get(handlers::health))
        .route("/awp/events/subscribe", post(handlers::subscribe))
        .route("/awp/events/subscriptions", get(handlers::list_subscriptions))
        .route(
            "/awp/events/subscriptions/{id}",
            delete(handlers::delete_subscription),
        )
        .route("/awp/a2a", post(routes::intent::a2a_intent))
        .layer(from_fn(version_negotiation))
        .with_state(state)
}