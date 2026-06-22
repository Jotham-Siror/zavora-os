use spatial_os::agents::deck::{self, McpPool};
use spatial_os::agents::morning::{self, MorningMcpPool};
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

    let mut deck_runner = None;
    let mut combine_runner = None;
    let mut deck_mcp = None;
    let mut deck_enabled = false;

    if config.agents_enabled() {
        match boot_deck_stack(&config, session_service.clone()).await {
            Ok((deck, combine, pool)) => {
                tracing::info!("Deck + combine workflows enabled (MCP + Gemini)");
                deck_runner = Some(deck);
                combine_runner = Some(combine);
                deck_mcp = Some(pool);
                deck_enabled = true;
            }
            Err(e) => {
                tracing::warn!("Deck workflow unavailable ({e:#}) — deck stays mock");
            }
        }
    } else {
        tracing::warn!("GOOGLE_API_KEY not set — deck uses mock SSE");
    }

    let mut morning_runner = None;
    let mut morning_mcp = None;
    let mut morning_enabled = false;

    if config.agents_enabled() {
        match boot_morning_stack(&config, session_service.clone()).await {
            Ok((runner, pool)) => {
                tracing::info!("Morning workflow enabled (news + weather MCP + Gemini)");
                morning_runner = Some(runner);
                morning_mcp = Some(pool);
                morning_enabled = true;
            }
            Err(e) => {
                tracing::warn!("Morning workflow unavailable ({e:#}) — morning stays mock");
            }
        }
    }

    let mut router_runner = None;
    let mut suzy_runner = None;
    let coordinator_enabled = config.agents_enabled();

    if coordinator_enabled {
        match boot_coordinator_stack(&config, session_service.clone()).await {
            Ok((router, suzy)) => {
                tracing::info!("Suzy coordinator + LLM router enabled");
                router_runner = Some(router);
                suzy_runner = Some(suzy);
            }
            Err(e) => {
                tracing::warn!("Coordinator unavailable ({e:#}) — keyword routing + static summaries");
            }
        }
    }

    let app_state = AppState::new(
        config.artifact_dir.clone(),
        deck_enabled,
        morning_enabled,
        coordinator_enabled,
        deck_runner,
        combine_runner,
        morning_runner,
        router_runner,
        suzy_runner,
        session_service,
        deck_mcp,
        morning_mcp,
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
        .route("/api/greeting", get(routes::greeting::get_greeting))
        .route("/api/oauth/{provider}", get(routes::oauth::oauth_guide))
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
        .route(
            "/api/sessions/{session_id}/cards",
            get(routes::cards::list_cards),
        )
        .route(
            "/api/sessions/{session_id}/agents",
            get(routes::agents::list_agents),
        )
        .route(
            "/api/sessions/{session_id}/commit",
            post(routes::commit::commit_action),
        )
        .route(
            "/api/agents/{agent_id}/snooze",
            post(routes::agents::snooze_agent),
        )
        .route(
            "/api/agents/{agent_id}/wake",
            post(routes::agents::wake_agent),
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
        .expect("agents_enabled implies API key");

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

async fn boot_morning_stack(
    config: &AppConfig,
    session_service: Arc<InMemorySessionService>,
) -> anyhow::Result<(Arc<Runner>, Arc<MorningMcpPool>)> {
    let api_key = config
        .google_api_key
        .as_deref()
        .expect("agents_enabled implies API key");

    let news = tools::mcp::spawn_mcp_server(&config.mcp_news_path).await?;
    let weather = tools::mcp::spawn_mcp_server(&config.mcp_weather_path).await?;
    let n = tools::mcp::health_check(&news).await?;
    let w = tools::mcp::health_check(&weather).await?;
    tracing::info!("Morning MCP ready: news={n}, weather={w}");

    let calendar = tools::mcp::try_spawn_mcp_server(&config.mcp_calendar_path)
        .await
        .map(|t| {
            tracing::info!("Calendar MCP connected");
            Arc::new(t) as Arc<dyn adk_core::Toolset>
        });
    let email = tools::mcp::try_spawn_mcp_server(&config.mcp_email_path)
        .await
        .map(|t| {
            tracing::info!("Email MCP connected");
            Arc::new(t) as Arc<dyn adk_core::Toolset>
        });

    if calendar.is_none() {
        tracing::warn!("Calendar MCP unavailable — Today card uses brief context only");
    }
    if email.is_none() {
        tracing::warn!("Email MCP unavailable — set SMTP/IMAP or run mcp-email auth gmail");
    }

    let pool = Arc::new(MorningMcpPool {
        calendar,
        email,
        news: Arc::new(news),
        weather: Arc::new(weather),
    });

    let workflow = morning::build_workflow(api_key, &config.gemini_model, pool.as_ref()).await?;

    let runner = Arc::new(
        Runner::builder()
            .app_name("zavora-os-morning")
            .agent(workflow)
            .session_service(session_service)
            .build()?,
    );

    Ok((runner, pool))
}

async fn boot_coordinator_stack(
    config: &AppConfig,
    session_service: Arc<InMemorySessionService>,
) -> anyhow::Result<(Arc<Runner>, Arc<Runner>)> {
    let api_key = config
        .google_api_key
        .as_deref()
        .expect("agents_enabled implies API key");

    let router_agent =
        spatial_os::agents::router::build(api_key, &config.gemini_model).await?;
    let suzy_agent = spatial_os::agents::suzy::build(api_key, &config.gemini_model).await?;

    let router_runner = Arc::new(
        Runner::builder()
            .app_name("zavora-os-router")
            .agent(router_agent)
            .session_service(session_service.clone())
            .build()?,
    );

    let suzy_runner = Arc::new(
        Runner::builder()
            .app_name("zavora-os-suzy")
            .agent(suzy_agent)
            .session_service(session_service)
            .build()?,
    );

    Ok((router_runner, suzy_runner))
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