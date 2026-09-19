//! Validation suite for Zavora OS M1 deck stack.
//!
//! Run: `cargo test --test validate`
//! Full E2E (slow, uses Gemini + MCP): `cargo test --test validate deck_workflow -- --ignored`

mod common;

use std::sync::Arc;

use adk_core::{Content, Llm, LlmRequest};
use adk_model::gemini::GeminiModel;
use futures::StreamExt;
use spatial_os::config::AppConfig;
use spatial_os::events::mock;
use spatial_os::scenarios;
use spatial_os::tools::mcp;

#[test]
fn gemini_model_configuration() {
    common::load_env();
    assert_eq!(common::gemini_model(), "gemini-3.1-flash-lite");
    assert_eq!(
        AppConfig::from_env().expect("config").gemini_model,
        "gemini-3.1-flash-lite"
    );
}

#[tokio::test]
async fn mock_deck_intent_maps_to_scenario_events() {
    assert_eq!(mock::pick_scenario("Build me a pitch deck"), "deck");
    let types = mock::preview_intent_event_types("Build me a pitch deck", 4).await;
    assert_eq!(types[0], "scenario");
    assert!(types.contains(&"card_spawn".to_string()));
}

#[tokio::test]
async fn mcp_servers_spawn_and_expose_tools() {
    let paths = common::mcp_paths();
    common::assert_mcp_binaries_exist(&paths);

    let worksheet = mcp::spawn_mcp_server(&paths.worksheet).await.expect("worksheet");
    let docx = mcp::spawn_mcp_server(&paths.docx).await.expect("docx");
    let slides = mcp::spawn_mcp_server(&paths.slides).await.expect("slides");

    let w = mcp::health_check(&worksheet).await.expect("worksheet health");
    let d = mcp::health_check(&docx).await.expect("docx health");
    let s = mcp::health_check(&slides).await.expect("slides health");

    assert!(w > 10, "worksheet should expose many tools, got {w}");
    assert!(d > 10, "docx should expose many tools, got {d}");
    assert!(s > 10, "slides should expose many tools, got {s}");
}

#[tokio::test]
async fn gemini_31_flash_lite_responds() {
    let api_key = common::google_api_key();
    let model_name = common::gemini_model();
    assert_eq!(model_name, "gemini-3.1-flash-lite");

    let model = GeminiModel::new(&api_key, &model_name).expect("gemini model");
    let request = LlmRequest::new(
        &model_name,
        vec![Content::new("user").with_text("Reply with exactly the word PONG.")],
    );

    let mut stream = model
        .generate_content(request, false)
        .await
        .expect("generate_content should succeed");

    let response = stream
        .next()
        .await
        .expect("one response chunk")
        .expect("response ok");

    let text = response
        .content
        .expect("content")
        .parts
        .iter()
        .filter_map(|p| match p {
            adk_core::Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<String>();

    assert!(
        text.to_uppercase().contains("PONG"),
        "expected PONG in model reply, got: {text}"
    );
}

#[test]
fn combine_action_routing_is_live_for_deck() {
    assert_eq!(scenarios::pick_action("combine"), Some("combine"));
    assert!(scenarios::action_is_live("combine", Some("deck"), true));
    assert!(!scenarios::action_is_live("combine", Some("morning"), true));
    use spatial_os::scenarios::ScenarioLiveFlags;
    assert!(scenarios::intent_is_live(
        "deck",
        ScenarioLiveFlags {
            deck: true,
            ..Default::default()
        }
    ));
    assert!(scenarios::intent_is_live(
        "morning",
        ScenarioLiveFlags {
            morning: true,
            ..Default::default()
        }
    ));
    assert!(scenarios::intent_is_live(
        "live",
        ScenarioLiveFlags {
            live: true,
            ..Default::default()
        }
    ));
}

#[tokio::test]
async fn mock_combine_action_stream_completes_with_events() {
    use std::time::Duration;

    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let mut stream = mock::stream_action("combine", Some("deck"), "combine");
        let mut count = 0usize;
        while let Some(item) = stream.next().await {
            let _ = item.expect("sse event");
            count += 1;
        }
        count
    })
    .await
    .expect("action stream should finish within 5s");

    assert!(
        result >= 3,
        "deck combine mock should emit conduct + deck_finish + done, got {result}"
    );
}

#[test]
fn tour_prompts_advance_in_order() {
    use spatial_os::scenarios::tour;

    assert_eq!(tour::next_scenario("morning"), Some("people"));
    assert_eq!(tour::action_prompt("deck"), Some("combine"));
    assert_eq!(tour::scenario_prompt("live"), Some("What is happening live"));
}

#[test]
fn keyword_router_still_maps_pitch_to_deck() {
    assert_eq!(mock::pick_scenario("pitch presentation"), "deck");
    assert_eq!(mock::pick_scenario("Start my day"), "morning");
}

#[tokio::test]
async fn router_agent_builds_with_gemini() {
    let api_key = common::google_api_key();
    let agent =
        spatial_os::agents::router::build(&api_key, &common::gemini_model())
            .await
            .expect("router should build");
    assert_eq!(agent.name(), "intent_router");
}

#[tokio::test]
async fn live_workflow_builds_with_news_mcp() {
    let api_key = common::google_api_key();
    let paths = common::mcp_paths();
    let news = mcp::spawn_mcp_server(&paths.news).await.expect("news");

    let pool = spatial_os::agents::live::LiveMcpPool {
        news: Arc::new(news),
        market_data: None,
    };

    spatial_os::agents::live::build_workflow(&api_key, &common::gemini_model(), &pool)
        .await
        .expect("live workflow should build");
}

#[tokio::test]
async fn suzy_agent_builds_with_gemini() {
    let api_key = common::google_api_key();
    let agent = spatial_os::agents::suzy::build(&api_key, &common::gemini_model())
        .await
        .expect("suzy should build");
    assert_eq!(agent.name(), "suzy_coordinator");
}

#[tokio::test]
async fn session_persistence_cards_and_agents() {
    let store = spatial_os::state::SessionStore::new();
    let record = store.create().await;
    let sid = record.session_id;

    store
        .set_scenario(&sid, "deck", Some("Build me a pitch deck"))
        .await;
    store
        .upsert_card(
            &sid,
            0,
            serde_json::json!({"glyph":"📊","title":"Auto-Excel","agent":"auto-excel"}),
            "resolved",
            Some(serde_json::json!({"big":"+38%","sub":"ready"})),
            false,
        )
        .await;
    store
        .agent_snooze(
            &sid,
            spatial_os::state::AgentRecord {
                id: "auto-excel".into(),
                title: "Auto-Excel".into(),
                glyph: "📊".into(),
                agent: "auto-excel".into(),
                rail: "resting".into(),
                domain: Default::default(),
            },
        )
        .await;

    let cards = store.list_cards(&sid).await.expect("cards");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].status, "resolved");

    let (active, resting) = store.list_agents(&sid).await.expect("agents");
    assert!(active.is_empty());
    assert_eq!(resting.len(), 1);

    let woke = store.agent_wake(&sid, "auto-excel").await.expect("wake");
    assert_eq!(woke.title, "Auto-Excel");
}

#[tokio::test]
async fn morning_phase_b_mcp_spawn() {
    let paths = common::mcp_paths();
    assert!(paths.news.exists(), "build mcp-news: cd mcp-servers/mcp-news && cargo build --release");
    assert!(
        paths.weather.exists(),
        "build mcp-weather: cd mcp-servers/mcp-weather && cargo build --release"
    );

    let news = mcp::spawn_mcp_server(&paths.news).await.expect("news");
    let weather = mcp::spawn_mcp_server(&paths.weather).await.expect("weather");
    assert!(mcp::health_check(&news).await.expect("news health") > 5);
    assert!(mcp::health_check(&weather).await.expect("weather health") > 3);
}

#[tokio::test]
async fn morning_workflow_builds_with_news_and_weather() {
    let api_key = common::google_api_key();
    let paths = common::mcp_paths();

    let news = Arc::new(mcp::spawn_mcp_server(&paths.news).await.unwrap());
    let weather = Arc::new(mcp::spawn_mcp_server(&paths.weather).await.unwrap());

    let pool = spatial_os::agents::morning::MorningMcpPool {
        calendar: None,
        email: None,
        news,
        weather,
    };

    spatial_os::agents::morning::build_workflow(&api_key, &common::gemini_model(), &pool)
        .await
        .expect("morning workflow should build");
}

#[tokio::test]
async fn combine_agent_builds_with_configured_model() {
    let api_key = common::google_api_key();
    let paths = common::mcp_paths();
    common::assert_mcp_binaries_exist(&paths);

    let slides = mcp::spawn_mcp_server(&paths.slides).await.expect("slides");
    spatial_os::agents::combine::build(&api_key, &common::gemini_model(), Arc::new(slides))
        .await
        .expect("combine agent should build");
}

#[tokio::test]
async fn greeting_brand_fallback_is_honest() {
    let payload = spatial_os::greeting::compose(
        None,
        None,
        "I'm synced and ready — tell me what you'd like to do.",
        Some("warm, confident"),
    )
    .await;
    assert_eq!(payload.source, "brand");
    assert!(!payload.full_text.to_lowercase().contains("meetings"));
    assert!(!payload.full_text.to_lowercase().contains("emails"));
    assert!(payload.full_text.contains("I'm synced and ready"));
    assert_eq!(payload.audio_clip, "/audio/greeting.wav");
}

#[tokio::test]
async fn greeting_agent_builds_with_gemini() {
    let api_key = common::google_api_key();
    spatial_os::greeting::agent::build(&api_key, &common::gemini_model())
        .await
        .expect("greeting agent should build");
}

fn awp_gate_fixture() -> spatial_os::awp_gate::AwpGate {
    use adk_awp::BusinessContextLoader;

    let path = common::manifest_dir().join("business.toml");
    let loader = BusinessContextLoader::from_file(&path).expect("business.toml");
    spatial_os::awp_gate::AwpGate::new(Some("validate-jwt-secret".into()), loader.context_ref())
}

#[tokio::test]
async fn dev_auth_jwt_unlocks_known_awp_capabilities() {
    common::load_env();
    let Ok(jwt_secret) = std::env::var("JWT_SECRET") else {
        return;
    };
    if std::env::var("DATABASE_URL").is_err() {
        return;
    }

    let pool = common::postgres_pool().await;
    let email = format!("dev-validate-{}@localhost", uuid::Uuid::new_v4());
    let user = spatial_os::db::create_user(&pool, &email, Some("Dev"), "dev", Some(&email), None)
        .await
        .expect("dev user");
    let token = spatial_os::auth::create_token(user.id, &jwt_secret).expect("jwt");

    let gate = awp_gate_fixture();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    gate.check(&headers, "action:sess-1", "submit_action")
        .await
        .expect("dev JWT should satisfy known capability");
}

#[test]
fn voice_state_boots_when_api_key_present() {
    common::load_env();
    let config = AppConfig::from_env().expect("config");
    let voice = spatial_os::voice::VoiceState::boot(&config);
    if config.voice_enabled() {
        assert!(voice.enabled);
        assert!(voice.model.is_some());
    } else {
        assert!(!voice.enabled);
    }
}

fn awp_test_state() -> adk_awp::AwpState {
    use adk_awp::{
        AwpState, BusinessContextLoader, DefaultTrustAssigner, InMemoryConsentService,
        InMemoryEventSubscriptionService, InMemoryRateLimiter,
    };
    use std::sync::Arc;

    let path = common::manifest_dir().join("business.toml");
    let loader = BusinessContextLoader::from_file(&path).expect("business.toml");
    let event_service = Arc::new(InMemoryEventSubscriptionService::new());
    AwpState::builder(loader.context_ref())
        .rate_limiter(Arc::new(InMemoryRateLimiter::new()))
        .consent_service(Arc::new(InMemoryConsentService::new()))
        .event_service(event_service)
        .trust_assigner(Arc::new(DefaultTrustAssigner))
        .build()
}

#[tokio::test]
async fn awp_discovery_document_valid() {
    use adk_awp::awp_routes;
    use awp_types::{AwpDiscoveryDocument, CURRENT_VERSION};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let app = awp_routes(awp_test_state());
    let response = app
        .oneshot(Request::get("/.well-known/awp.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 16_384)
        .await
        .unwrap();
    let doc: AwpDiscoveryDocument = serde_json::from_slice(&body).unwrap();
    assert_eq!(doc.version, CURRENT_VERSION);
    assert!(doc.capability_manifest_url.contains("/awp/manifest"));
    assert!(doc.a2a_endpoint_url.contains("/awp/a2a"));
}

#[tokio::test]
async fn awp_manifest_lists_submit_intent() {
    use adk_awp::awp_routes;
    use awp_types::CapabilityManifest;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let app = awp_routes(awp_test_state());
    let response = app
        .oneshot(Request::get("/awp/manifest").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 65_536)
        .await
        .unwrap();
    let manifest: CapabilityManifest = serde_json::from_slice(&body).unwrap();
    assert!(manifest.capabilities.iter().any(|c| c.name == "submit_intent"));
    assert!(manifest.capabilities.iter().any(|c| c.name == "stream_voice"));
}

#[tokio::test]
#[ignore = "set ZAVORA_DEPLOY_URL=https://your-host to run production AWP conformance"]
async fn awp_conformance_against_deploy_url() {
    let base = std::env::var("ZAVORA_DEPLOY_URL").expect("ZAVORA_DEPLOY_URL");
    let client = reqwest::Client::new();
    let doc: serde_json::Value = client
        .get(format!("{base}/.well-known/awp.json"))
        .send()
        .await
        .expect("discovery request")
        .error_for_status()
        .expect("discovery status")
        .json()
        .await
        .expect("discovery json");
    assert!(doc.get("version").is_some(), "discovery missing version");

    let manifest: serde_json::Value = client
        .get(format!("{base}/awp/manifest"))
        .send()
        .await
        .expect("manifest request")
        .error_for_status()
        .expect("manifest status")
        .json()
        .await
        .expect("manifest json");
    let caps = manifest
        .get("capabilities")
        .and_then(|c| c.as_array())
        .expect("capabilities array");
    assert!(
        caps.iter()
            .any(|c| c.get("name").and_then(|n| n.as_str()) == Some("submit_intent")),
        "manifest missing submit_intent"
    );
}


/// Offline `AppState`: no API key, no MCP, in-memory sessions — every scenario streams its mock.
fn offline_app_state() -> spatial_os::state::AppState {
    use adk_awp::BusinessContextLoader;
    use std::sync::Arc;

    common::load_env();
    let config = AppConfig::from_env().expect("config");
    let path = common::manifest_dir().join("business.toml");
    let loader = BusinessContextLoader::from_file(&path).expect("business.toml");
    let awp = Arc::new(spatial_os::awp_gate::AwpGate::new(
        config.jwt_secret.clone(),
        loader.context_ref(),
    ));
    let runtime = spatial_os::state::RuntimeStatus {
        milestone: "M11",
        phase: "P2-S0",
        agents_enabled: config.agents_enabled(),
        postgres_enabled: config.postgres_enabled(),
        auth_enabled: config.auth_enabled(),
        voice_enabled: config.voice_enabled(),
        coordinator_enabled: config.agents_enabled(),
        uses_mock_orchestration: !config.agents_enabled(),
        scenarios: spatial_os::scenarios::ScenarioLiveFlags::default(),
        mcp_worksheet: false,
        mcp_docx: false,
        mcp_slides: false,
        mcp_news: false,
        allow_demo_mode: config.allow_demo_mode,
        public_domain: config.public_domain(),
        signup_endpoint: config.signup_endpoint.clone(),
        linkedin_partner_id: config.linkedin_partner_id.clone(),
        linkedin_conversion_id: config.linkedin_conversion_id,
    };
    spatial_os::state::AppState::new(
        runtime,
        spatial_os::state::SessionStore::new(),
        config.artifact_dir.clone(),
        spatial_os::scenarios::ScenarioLiveFlags::default(),
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Arc::new({
            use adk_session::InMemorySessionService;
            InMemorySessionService::new()
        }),
        None,
        awp,
        Arc::new(adk_awp::InMemoryEventSubscriptionService::new()),
        None,
        None,
        None,
        None,
        None,
        None,
        spatial_os::ambient::AmbientStore::new(),
        false,
        None,
        "test".into(),
        None,
        spatial_os::voice::VoiceState::boot(&config),
    )
}

#[tokio::test]
async fn health_exposes_runtime_status() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    common::load_env();
    let state = offline_app_state();
    let app = axum::Router::new()
        .route("/health", axum::routing::get(spatial_os::routes::health::health))
        .with_state(state);
    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["runtime"]["milestone"], "M11");
}

#[test]
fn business_toml_lists_voice_capabilities() {
    use adk_awp::BusinessContextLoader;

    let path = common::manifest_dir().join("business.toml");
    let loader = BusinessContextLoader::from_file(&path).expect("business.toml");
    let ctx = loader.load();
    let caps = &ctx.capabilities;
    let names: Vec<&str> = caps.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"stream_voice"), "missing stream_voice capability");
    assert!(names.contains(&"voice_status"), "missing voice_status capability");

    let stream = caps.iter().find(|c| c.name == "stream_voice").expect("stream_voice");
    assert_eq!(stream.endpoint, "/ws/voice");
    let status = caps.iter().find(|c| c.name == "voice_status").expect("voice_status");
    assert_eq!(status.endpoint, "/api/voice/status");
}

#[test]
fn mcp_allowlist_catalog_covers_deck_agents() {
    let path = common::manifest_dir().join("mcp_allowlists.toml");
    let catalog = spatial_os::tools::allowlist::AllowlistCatalog::from_file(&path)
        .expect("mcp_allowlists.toml");

    for agent in [
        "excel_agent",
        "docs_agent",
        "slides_agent",
        "combine_agent",
        "brief_agent",
        "headlines_agent",
    ] {
        assert!(
            catalog.contains_agent(agent),
            "missing allowlist for {agent}"
        );
        assert!(
            !catalog.tools_for_agent(agent).is_empty(),
            "empty tools for {agent}"
        );
    }

    assert_eq!(catalog.tools_for_agent("excel_agent").len(), 10);
    assert!(catalog.tools_for_agent("brief_agent").contains(&"get_forecast".to_string()));
}

#[tokio::test]
async fn awp_gate_allows_anonymous_intent() {
    use axum::http::HeaderMap;

    let gate = awp_gate_fixture();
    let headers = HeaderMap::new();
    let trust = gate
        .check(&headers, "intent:sess-1", "submit_intent")
        .await
        .expect("anonymous intent should pass");
    assert_eq!(trust, awp_types::TrustLevel::Anonymous);
}

#[tokio::test]
async fn awp_gate_blocks_anonymous_action() {
    use axum::http::HeaderMap;

    let gate = awp_gate_fixture();
    let headers = HeaderMap::new();
    let err = gate
        .check(&headers, "action:sess-1", "submit_action")
        .await
        .expect_err("anonymous action should be forbidden");
    assert_eq!(err.status(), axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn awp_gate_jwt_bearer_unlocks_known_capabilities() {
    use axum::http::HeaderMap;
    use spatial_os::auth;

    let secret = "validate-jwt-secret";
    let gate = awp_gate_fixture();
    let token = auth::create_token(uuid::Uuid::new_v4(), secret).expect("jwt");

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );

    let trust = gate
        .check(&headers, "action:sess-1", "submit_action")
        .await
        .expect("known caller should pass submit_action");
    assert_eq!(trust, awp_types::TrustLevel::Known);
}

#[tokio::test]
async fn awp_gate_blocks_anonymous_subscribe() {
    use axum::http::HeaderMap;

    let gate = awp_gate_fixture();
    let headers = HeaderMap::new();
    let err = gate
        .check(&headers, "awp-subscribe", "subscribe_proactive")
        .await
        .expect_err("anonymous subscribe should be forbidden");
    assert_eq!(err.status(), axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn session_store_create_for_user() {
    let store = spatial_os::state::SessionStore::new();
    let record = store.create_for_user("user-abc".into()).await;
    assert_eq!(record.user_id, "user-abc");
    let loaded = store.get(&record.session_id).await.expect("session");
    assert_eq!(loaded.user_id, "user-abc");
}

#[tokio::test]
async fn postgres_schema_has_m9_tables() {
    let pool = common::postgres_pool().await;

    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename",
    )
    .fetch_all(&pool)
    .await
    .expect("list tables");

    let names: Vec<&str> = tables.iter().map(|(t,)| t.as_str()).collect();
    for expected in [
        "_sqlx_migrations",
        "agent_events",
        "agent_sessions",
        "ui_sessions",
        "users",
    ] {
        assert!(names.contains(&expected), "missing table {expected}, got {names:?}");
    }

    let migrations: Vec<(i64, String)> =
        sqlx::query_as("SELECT version, description FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .expect("migrations");
    assert!(
        migrations.len() >= 3,
        "expected at least 3 migrations, got {migrations:?}"
    );
}

#[tokio::test]
async fn ui_session_persists_across_store_instances() {
    use spatial_os::state::{AgentRecord, SessionStore};

    let pool = common::postgres_pool().await;
    let store_a = SessionStore::with_postgres(pool.clone());
    assert!(store_a.postgres_enabled());

    let record = store_a.create_for_user("user-validate".into()).await;
    store_a
        .set_scenario(&record.session_id, "deck", Some("Build me a pitch deck"))
        .await;
    store_a
        .agent_active(
            &record.session_id,
            AgentRecord {
                id: "deck.agent".into(),
                title: "Deck".into(),
                glyph: "📊".into(),
                agent: "deck.agent".into(),
                rail: "active".into(),
                domain: Default::default(),
            },
        )
        .await;
    store_a
        .upsert_card(
            &record.session_id,
            0,
            serde_json::json!({"title": "Excel", "glyph": "📗"}),
            "resolved",
            Some(serde_json::json!({"big": "Done"})),
            false,
        )
        .await;

    let store_b = SessionStore::with_postgres(pool.clone());
    let loaded = store_b
        .get(&record.session_id)
        .await
        .expect("session should load from postgres via fresh store");
    assert_eq!(loaded.user_id, "user-validate");
    assert_eq!(loaded.scenario.as_deref(), Some("deck"));
    assert_eq!(loaded.origin_text.as_deref(), Some("Build me a pitch deck"));
    assert_eq!(loaded.agents_active.len(), 1);
    assert_eq!(loaded.agents_active[0].id, "deck.agent");
    assert_eq!(loaded.cards.len(), 1);
    assert_eq!(
        loaded.cards[0].card.get("title").and_then(|v| v.as_str()),
        Some("Excel")
    );

    let row: (String, serde_json::Value) = sqlx::query_as(
        "SELECT user_id, cards FROM ui_sessions WHERE session_id = $1",
    )
    .bind(&record.session_id)
    .fetch_one(&pool)
    .await
    .expect("ui_sessions row");
    assert_eq!(row.0, "user-validate");
    assert!(row.1.is_array());
}

#[tokio::test]
async fn artifact_paths_are_user_scoped() {
    use spatial_os::artifacts;

    let root = std::path::Path::new("/tmp/zavora-artifacts");
    let user = "user-1";
    let session = "sess-1";
    let dir = artifacts::session_dir(root, user, session);
    assert!(dir.ends_with("user-1/sess-1"));
    let file = dir.join("pitch_deck.pptx");
    let url = artifacts::public_url(user, session, &file, root).expect("url");
    assert_eq!(url, "/artifacts/user-1/sess-1/pitch_deck.pptx");
    let outside = root.join("other/file");
    assert!(artifacts::public_url(user, session, &outside, root).is_none());
}

#[tokio::test]
async fn pg_agent_session_roundtrip() {
    use adk_session::{CreateRequest, GetRequest, SessionService};
    use spatial_os::pg_session::PgSessionService;

    let pool = common::postgres_pool().await;
    let svc = PgSessionService::new(pool);

    let session_id = uuid::Uuid::new_v4().to_string();
    let user_id = uuid::Uuid::new_v4().to_string();

    svc.create(CreateRequest {
        app_name: "zavora-os-validate".into(),
        user_id: user_id.clone(),
        session_id: Some(session_id.clone()),
        state: Default::default(),
    })
    .await
    .expect("create agent session");

    let loaded = svc
        .get(GetRequest {
            app_name: "zavora-os-validate".into(),
            user_id,
            session_id,
            num_recent_events: None,
            after: None,
        })
        .await
        .expect("get agent session");

    assert_eq!(loaded.app_name(), "zavora-os-validate");
}

#[tokio::test]
async fn people_rail_unavailable_without_slack() {
    let rail = spatial_os::rails::people::fetch(None).await;
    assert_eq!(rail.source, "unavailable");
    assert!(rail.work.is_empty());
    assert!(rail.family.is_empty());
    assert!(rail.message.as_deref().unwrap_or("").contains("Slack"));
}

#[tokio::test]
async fn live_slides_from_mcp_news() {
    let paths = common::mcp_paths();
    common::assert_mcp_binaries_exist(&paths);

    let news = Arc::new(mcp::spawn_mcp_server(&paths.news).await.expect("news mcp"));
    let (source, slides) = spatial_os::rails::live::fetch_slides(news).await;
    assert!(
        source == "mcp-news" || source == "unavailable",
        "unexpected source: {source}"
    );
    if source == "mcp-news" {
        assert!(!slides.is_empty(), "expected headlines from hn_stories");
        assert!(!slides[0].body.is_empty());
    }
}

#[tokio::test]
async fn background_cards_reflect_empty_integrations() {
    use spatial_os::rails::background;

    let cards = background::build(&[], &[], &[], &[]);
    assert!(!cards.is_empty(), "should still emit flank card shells");
    let people = cards.iter().find(|c| c.kind == "people").expect("people card");
    assert_eq!(people.title, "People");
    assert!(people.rows.is_empty());
    let live = cards.iter().find(|c| c.kind == "live").expect("live card");
    assert_eq!(live.sub, "headlines");
}

#[tokio::test]
async fn ambient_store_tracks_agent_lifecycle() {
    use spatial_os::ambient::AmbientStore;

    let store = AmbientStore::new();
    store.set_working("research", "reading sources").await;
    let rec = store.get("research").await.expect("research agent");
    assert_eq!(rec.status, "working");
    assert_eq!(rec.task, "reading sources");

    store
        .set_done(
            "research",
            "ABC Corp brief",
            serde_json::json!({"big": "Brief ready", "sub": "done"}),
            Some("summary".into()),
        )
        .await;
    let rec = store.get("research").await.expect("research done");
    assert_eq!(rec.status, "done");
    assert!(rec.resolve.is_some());

    store.set_dnd(true).await;
    assert!(store.dnd().await);
}

#[tokio::test]
async fn proactive_mock_scenario_has_three_cards() {
    assert_eq!(mock::pick_scenario("Show me what you found"), "proactive");
    let types = mock::preview_intent_event_types("Show me what you found", 6).await;
    assert_eq!(types[0], "scenario");
    assert!(types.iter().filter(|t| *t == "card_spawn").count() >= 3);
}

#[tokio::test]
async fn ambient_agents_build_with_configured_model() {
    let api_key = common::google_api_key();
    let paths = common::mcp_paths();
    let news = Arc::new(mcp::spawn_mcp_server(&paths.news).await.expect("news"));

    spatial_os::agents::ambient::research::build(&api_key, &common::gemini_model(), news.clone())
        .await
        .expect("research agent");
    spatial_os::agents::ambient::scout::build(&api_key, &common::gemini_model(), None)
        .await
        .expect("scout agent");
    spatial_os::agents::ambient::maker::build(&api_key, &common::gemini_model())
        .await
        .expect("maker agent");
}

#[tokio::test]
async fn deck_agents_build_with_configured_model() {
    let api_key = common::google_api_key();
    let paths = common::mcp_paths();
    common::assert_mcp_binaries_exist(&paths);

    let pool = spatial_os::agents::deck::McpPool {
        worksheet: Arc::new(mcp::spawn_mcp_server(&paths.worksheet).await.unwrap()),
        docx: Arc::new(mcp::spawn_mcp_server(&paths.docx).await.unwrap()),
        slides: Arc::new(mcp::spawn_mcp_server(&paths.slides).await.unwrap()),
    };

    spatial_os::agents::deck::build_workflow(&api_key, &common::gemini_model(), &pool)
        .await
        .expect("deck workflow should build");
}

#[tokio::test]
#[ignore = "slow: full deck E2E (~2-5 min). Run: cargo test --test validate deck_workflow -- --ignored"]
async fn deck_workflow_writes_three_artifacts() {
    use adk_core::{SessionId, UserId};
    use adk_runner::Runner;
    use adk_session::{CreateRequest, InMemorySessionService, SessionService};

    let api_key = common::google_api_key();
    let paths = common::mcp_paths();
    common::assert_mcp_binaries_exist(&paths);

    let pool = spatial_os::agents::deck::McpPool {
        worksheet: Arc::new(mcp::spawn_mcp_server(&paths.worksheet).await.unwrap()),
        docx: Arc::new(mcp::spawn_mcp_server(&paths.docx).await.unwrap()),
        slides: Arc::new(mcp::spawn_mcp_server(&paths.slides).await.unwrap()),
    };

    let workflow =
        spatial_os::agents::deck::build_workflow(&api_key, &common::gemini_model(), &pool)
            .await
            .unwrap();

    let session_service = Arc::new(InMemorySessionService::new());
    let runner = Runner::builder()
        .app_name("zavora-os-validate")
        .agent(workflow)
        .session_service(session_service.clone())
        .build()
        .unwrap();

    let session_id = uuid::Uuid::new_v4().to_string();
    let user_id = uuid::Uuid::new_v4().to_string();
    session_service
        .create(CreateRequest {
            app_name: "zavora-os-validate".into(),
            user_id: user_id.clone(),
            session_id: Some(session_id.clone()),
            state: Default::default(),
        })
        .await
        .unwrap();

    let artifact_root = tempfile::tempdir().unwrap();
    let session_dir = artifact_root.path().join(&session_id);
    std::fs::create_dir_all(&session_dir).unwrap();

    let prompt = format!(
        "Build me a pitch deck\n\n[Save files to: {}/]\n[Sibling artifacts]\n(none yet)\n",
        session_dir.display()
    );

    let mut stream = runner
        .run(
            UserId::try_from(user_id.as_str()).unwrap(),
            SessionId::try_from(session_id.as_str()).unwrap(),
            Content::new("user").with_text(&prompt),
        )
        .await
        .unwrap();

    while let Some(ev) = stream.next().await {
        ev.expect("runner event");
    }

    for ext in ["xlsx", "docx", "pptx"] {
        let found = std::fs::read_dir(&session_dir)
            .unwrap()
            .flatten()
            .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some(ext));
        assert!(found, "expected .{ext} in {}", session_dir.display());
    }
}
// ---------------------------------------------------------------------------
// Phase 2 · S0 — domain model and agent contract
// ---------------------------------------------------------------------------

#[test]
fn domain_card_spawn_event_carries_domain() {
    use spatial_os::domain::Domain;
    use spatial_os::events::sse::FieldEvent;

    let card = serde_json::json!({"glyph":"✉️","title":"Needs you","agent":"inbox.agent"});
    let ev = FieldEvent::CardSpawn {
        index: 1,
        card: card.clone(),
        domain: Domain::for_card("morning", &card),
    };
    let json = serde_json::to_value(&ev).expect("serialize");
    assert_eq!(json["type"], "card_spawn");
    assert_eq!(json["domain"], "work");

    // Phase 1 JSON without a domain still deserializes (default = shared).
    let legacy = r#"{"index":0,"card":{},"status":"spawn","resolve":null,"pinned":false,"removed":false}"#;
    let rec: spatial_os::state::CardRecord = serde_json::from_str(legacy).expect("legacy card");
    assert_eq!(rec.domain, Domain::Shared);
}

#[tokio::test]
async fn domain_is_derived_when_cards_are_persisted() {
    use spatial_os::domain::Domain;
    let store = spatial_os::state::SessionStore::new();
    let rec = store.create_for_user("u-domain".into()).await;
    store.set_scenario(&rec.session_id, "deck", Some("Build me a pitch deck")).await;
    store
        .upsert_card(&rec.session_id, 0, serde_json::json!({"title":"Auto-Excel","agent":"auto-excel"}), "spawn", None, false)
        .await;
    store
        .upsert_card(&rec.session_id, 1, serde_json::json!({"title":"Family","agent":"x","domain":"home"}), "spawn", None, false)
        .await;
    let cards = store.list_cards(&rec.session_id).await.expect("cards");
    assert_eq!(cards[0].domain, Domain::Work);
    assert_eq!(cards[1].domain, Domain::Home);
}

#[test]
fn mcp_allowlist_specs_declare_world_and_mode() {
    use spatial_os::domain::Domain;
    use spatial_os::permissions::Mode;
    use spatial_os::tools::allowlist::AllowlistCatalog;

    let path = common::manifest_dir().join("mcp_allowlists.toml");
    let catalog = AllowlistCatalog::from_file(&path).expect("catalog");
    let inbox = catalog.spec_for("inbox_agent").expect("inbox spec");
    assert_eq!(inbox.world, Domain::Work);
    assert_eq!(inbox.mode, Mode::Suggest);
    assert!(inbox.tool_names().contains(&"create_draft".to_string()));

    let money = catalog.spec_for("money_agent").expect("money spec");
    assert_eq!(money.world, Domain::Home);
    assert_eq!(money.mode, Mode::Observe);

    // Unknown agents fall back to the safe defaults.
    assert_eq!(catalog.mode_for("nobody"), Mode::Suggest);
    assert_eq!(catalog.world_for("nobody"), Domain::Shared);
    // S0 only reports missing effects; S2 makes them a boot failure.
    catalog.validate_effects(false).expect("lenient validation");
    assert!(catalog.effects_for_unknown_tools().is_empty());
}

#[test]
fn mcp_allowlist_schema_is_backward_compatible() {
    use spatial_os::tools::allowlist::AllowlistCatalog;
    let legacy: Vec<spatial_os::tools::allowlist::AllowlistEntry> = toml::from_str::<toml::Value>(
        r#"
[[allowlist]]
agent = "legacy_agent"
mcp_server = "news"
tools = ["search_news"]
"#,
    )
    .expect("toml")
    .get("allowlist")
    .cloned()
    .expect("array")
    .try_into()
    .expect("entries");
    let catalog = AllowlistCatalog::from_entries(legacy);
    let spec = catalog.spec_for("legacy_agent").expect("spec");
    assert_eq!(spec.mode, spatial_os::permissions::Mode::Suggest);
    assert_eq!(spec.world, spatial_os::domain::Domain::Shared);
    assert_eq!(catalog.tools_missing_effects(), vec![("legacy_agent".to_string(), "search_news".to_string())]);
}


// ---------------------------------------------------------------------------
// Phase 2 · S1 — Mother Agent
// ---------------------------------------------------------------------------

fn ev_types(events: &[serde_json::Value]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| e.get("type").and_then(|t| t.as_str()).map(str::to_string))
        .collect()
}

#[tokio::test]
async fn mother_multi_target_merges_two_scenarios_offline() {
    use spatial_os::orchestrator::dispatch::{dispatch_intent, IntentDispatch};
    use spatial_os::orchestrator::sse_collect::collect_sse_events;

    let state = offline_app_state();
    let rec = state.sessions.create_for_user("u-mother".into()).await;
    let response = dispatch_intent(IntentDispatch {
        state: &state,
        session_id: rec.session_id.clone(),
        user_id: rec.user_id.clone(),
        text: "What's happening with work?".into(),
    })
    .await;
    let events = collect_sse_events(response).await;
    let types = ev_types(&events);

    // One scenario header covering both delegated workflows (morning 3 cards + people 3 cards).
    assert_eq!(types.iter().filter(|t| *t == "scenario").count(), 1, "{types:?}");
    assert_eq!(events[0]["total_cards"], 6);
    let spawns: Vec<u64> = events
        .iter()
        .filter(|e| e["type"] == "card_spawn")
        .map(|e| e["index"].as_u64().unwrap())
        .collect();
    assert_eq!(spawns, vec![0, 1, 2, 3, 4, 5], "cards must be re-indexed across targets");
    assert!(events.iter().filter(|e| e["type"] == "card_spawn").all(|e| e["domain"].is_string()));
    assert_eq!(types.iter().filter(|t| *t == "card_resolve").count(), 6);
    // Exactly one synthesis from the Mother, then done.
    assert_eq!(types.iter().filter(|t| *t == "suzy_summary").count(), 1);
    let summary = events.iter().find(|e| e["type"] == "suzy_summary").unwrap();
    assert_eq!(summary["key"], "mother");
    assert!(summary["html"].as_str().unwrap().contains("Work:"), "{}", summary["html"]);
    assert_eq!(types.last().map(String::as_str), Some("done"));
    // Suggested actions carry a mode badge.
    assert!(events.iter().any(|e| e["type"] == "suggest" && e["text"].as_str().unwrap().starts_with(|c: char| "👁💡⚡".contains(c))));

    // Merged cards were persisted under the primary scenario with re-indexed positions.
    let cards = state.sessions.list_cards(&rec.session_id).await.expect("cards");
    assert_eq!(cards.len(), 6);
    assert!(cards.iter().all(|c| c.resolve.is_some()));
    let stored = state.sessions.get(&rec.session_id).await.unwrap();
    assert_eq!(stored.scenario.as_deref(), Some("morning"));
    assert!(stored.chat_history.iter().any(|t| t.role == "mother"));
}

#[tokio::test]
async fn mother_single_target_passes_phase1_scenario_through() {
    use spatial_os::orchestrator::dispatch::{dispatch_intent, IntentDispatch};
    use spatial_os::orchestrator::sse_collect::collect_sse_events;

    let state = offline_app_state();
    let rec = state.sessions.create_for_user("u-deck".into()).await;
    let response = dispatch_intent(IntentDispatch {
        state: &state,
        session_id: rec.session_id.clone(),
        user_id: rec.user_id.clone(),
        text: "Build me a pitch deck".into(),
    })
    .await;
    let events = collect_sse_events(response).await;
    assert_eq!(events[0]["type"], "scenario");
    assert_eq!(events[0]["key"], "deck");
    let cards = state.sessions.list_cards(&rec.session_id).await.expect("cards");
    assert!(cards.len() >= 3, "deck scenario persists its cards");
    assert_eq!(events[0]["total_cards"].as_u64().unwrap() as usize, cards.len());
    assert!(cards.iter().all(|c| c.domain == spatial_os::domain::Domain::Work));
}

#[tokio::test]
async fn mother_clarifies_ambiguous_intent() {
    use spatial_os::orchestrator::dispatch::{dispatch_intent, IntentDispatch};
    use spatial_os::orchestrator::sse_collect::collect_sse_events;

    let state = offline_app_state();
    let rec = state.sessions.create_for_user("u-hmm".into()).await;
    let response = dispatch_intent(IntentDispatch {
        state: &state,
        session_id: rec.session_id.clone(),
        user_id: rec.user_id.clone(),
        text: "hmm".into(),
    })
    .await;
    let events = collect_sse_events(response).await;
    let types = ev_types(&events);
    assert_eq!(types[0], "suzy_summary");
    assert_eq!(events[0]["key"], "clarify");
    assert!(types.iter().filter(|t| *t == "suggest").count() >= 3);
    assert!(!types.contains(&"card_spawn".to_string()));
}

#[test]
fn mother_merge_reindexes_and_drops_inner_wrappers() {
    use spatial_os::domain::Domain;
    use spatial_os::mother::delegate::merge_target_events;
    use spatial_os::mother::intake::Target;

    let targets = vec![
        Target { world: Domain::Work, agent: "productivity".into(), task: "t".into(), scenario: "morning".into() },
        Target { world: Domain::Home, agent: "family".into(), task: "t".into(), scenario: "people".into() },
    ];
    let a = vec![
        serde_json::json!({"type":"scenario","key":"morning","text":"x","total_cards":2}),
        serde_json::json!({"type":"card_spawn","index":0,"card":{"title":"Today","agent":"calendar.agent"},"domain":"work"}),
        serde_json::json!({"type":"card_spawn","index":1,"card":{"title":"Needs you","agent":"inbox.agent"},"domain":"work"}),
        serde_json::json!({"type":"card_resolve","index":1,"resolve":{"big":"2 to reply","actions":["Draft replies"]}}),
        serde_json::json!({"type":"suzy_summary","key":"morning","html":"inner"}),
        serde_json::json!({"type":"done"}),
    ];
    let b = vec![
        serde_json::json!({"type":"scenario","key":"people","text":"x","total_cards":1}),
        serde_json::json!({"type":"card_spawn","index":0,"card":{"title":"Connections","agent":"crm.agent"}}),
        serde_json::json!({"type":"card_status","index":0,"status":"working","line":"Finding…"}),
        serde_json::json!({"type":"card_resolve","index":0,"resolve":{"lines":["Birthday: Mara"],"actions":["Send notes"]}}),
        serde_json::json!({"type":"done"}),
    ];
    let merged = merge_target_events(&targets, vec![(a, false), (b, true)]);
    assert_eq!(merged.total_cards, 3);
    let json: Vec<serde_json::Value> = merged.events.iter().map(|e| serde_json::to_value(e).unwrap()).collect();
    let types = ev_types(&json);
    assert!(!types.iter().any(|t| t == "scenario" || t == "suzy_summary" || t == "done"));
    let third_spawn = json.iter().find(|e| e["type"] == "card_spawn" && e["index"] == 2).expect("re-indexed spawn");
    assert_eq!(third_spawn["card"]["title"], "Connections");
    // The people-scenario card inherits the Home target's world when the card itself is unspecific.
    assert_eq!(third_spawn["domain"], "home");
    assert!(json.iter().any(|e| e["type"] == "card_status" && e["index"] == 2));
    assert_eq!(merged.results.len(), 2);
    assert!(merged.results[1].timed_out);
    assert_eq!(merged.results[0].cards[1].resolve.as_ref().unwrap()["big"], "2 to reply");
    assert_eq!(merged.card_at(2).unwrap()["title"], "Connections");
}

#[tokio::test]
async fn mother_chat_route_streams_and_records_history() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use spatial_os::orchestrator::sse_collect::collect_sse_events;
    use tower::ServiceExt;

    let state = offline_app_state();
    let rec = state.sessions.create_for_user("u-chat".into()).await;
    let app = axum::Router::new()
        .route(
            "/api/sessions/{session_id}/chat",
            axum::routing::post(spatial_os::routes::chat::chat).get(spatial_os::routes::chat::history),
        )
        .with_state(state.clone());

    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/sessions/{}/chat", rec.session_id))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"text":"Prepare me for my afternoon."}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let events = collect_sse_events(response).await;
    assert_eq!(events[0]["type"], "scenario");
    assert_eq!(events[0]["total_cards"], 6);

    let response = app
        .oneshot(Request::get(format!("/api/sessions/{}/chat", rec.session_id)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let turns = json["turns"].as_array().unwrap();
    assert_eq!(turns[0]["role"], "user");
    assert_eq!(turns[0]["text"], "Prepare me for my afternoon.");
    assert_eq!(turns.last().unwrap()["role"], "mother");
}

#[test]
fn business_toml_lists_chat_mother_capability() {
    use adk_awp::BusinessContextLoader;
    let path = common::manifest_dir().join("business.toml");
    let ctx = BusinessContextLoader::from_file(&path).expect("business.toml").load();
    assert!(ctx.capabilities.iter().any(|c| c.name == "chat_mother" && c.endpoint.contains("/chat")));
}
