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