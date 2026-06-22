use spatial_os::agents::deck::{self, McpPool};
use spatial_os::config::AppConfig;
use spatial_os::routes;
use spatial_os::state::AppState;
use spatial_os::tools;

use std::path::Path;
use std::sync::Arc;

use adk_awp::{
    handlers, middleware::version_negotiation, AwpState, BusinessContextLoader,
    DefaultTrustAssigner, HealthStateMachine, InMemoryConsentService,
    InMemoryEventSubscriptionService, InMemoryRateLimiter,
};
use adk_runner::Runner;
use adk_session::InMemorySessionService;
use axum::middleware::from_fn;
use axum::routing::{delete, get, post};
use axum::{Extension, Router};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = AppConfig::from_env()?;
    tokio::fs::create_dir_all(&config.artifact_dir).await?;

    let session_service = Arc::new(InMemorySessionService::new());

    let (deck_runner, combine_runner, mcp_pool, deck_enabled) = if config.deck_enabled() {
        match boot_deck_stack(&config, session_service.clone()).await {
            Ok((deck_runner, combine_runner, pool)) => {
                tracing::info!("Deck + combine workflows enabled (MCP + Gemini)");
                (Some(deck_runner), Some(combine_runner), Some(pool), true)
            }
            Err(e) => {
                tracing::warn!("Deck workflow unavailable ({e:#}) — mock scenarios only");
                (None, None, None, false)
            }
        }
    } else {
        tracing::warn!("GOOGLE_API_KEY not set — deck uses mock SSE");
        (None, None, None, false)
    };

    let app_state = AppState::new(
        config.artifact_dir.clone(),
        deck_enabled,
        deck_runner,
        combine_runner,
        session_service,
        mcp_pool,
    );
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
        .route(
            "/api/sessions/{session_id}/action",
            post(routes::action::submit_action),
        )
        .route(
            "/api/sessions/{session_id}/fuse",
            post(routes::fuse::fuse_cards),
        )
        .with_state(app_state)
        .merge(awp_router(awp_state))
        .layer(CorsLayer::very_permissive());

    let mut app = api.layer(Extension(session_store));

    if Path::new(&config.artifact_dir).exists() {
        app = app.nest_service("/artifacts", ServeDir::new(&config.artifact_dir));
    }

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

async fn boot_deck_stack(
    config: &AppConfig,
    session_service: Arc<InMemorySessionService>,
) -> anyhow::Result<(Arc<Runner>, Arc<Runner>, Arc<McpPool>)> {
    let api_key = config
        .google_api_key
        .as_deref()
        .expect("deck_enabled implies API key");

    let worksheet = tools::mcp::spawn_mcp_server(&config.mcp_worksheet_path).await?;
    let docx = tools::mcp::spawn_mcp_server(&config.mcp_docx_path).await?;
    let slides = tools::mcp::spawn_mcp_server(&config.mcp_slides_path).await?;

    let w = tools::mcp::health_check(&worksheet).await?;
    let d = tools::mcp::health_check(&docx).await?;
    let s = tools::mcp::health_check(&slides).await?;
    tracing::info!("MCP tools ready: worksheet={w}, docx={d}, slides={s}");

    let pool = Arc::new(McpPool {
        worksheet: Arc::new(worksheet),
        docx: Arc::new(docx),
        slides: Arc::new(slides),
    });

    let workflow = deck::build_workflow(api_key, &config.gemini_model, pool.as_ref()).await?;
    let combine_agent =
        spatial_os::agents::combine::build(api_key, &config.gemini_model, pool.slides.clone())
            .await?;

    let deck_runner = Arc::new(
        Runner::builder()
            .app_name("zavora-os")
            .agent(workflow)
            .session_service(session_service.clone())
            .build()?,
    );

    let combine_runner = Arc::new(
        Runner::builder()
            .app_name("zavora-os-combine")
            .agent(combine_agent)
            .session_service(session_service)
            .build()?,
    );

    Ok((deck_runner, combine_runner, pool))
}

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