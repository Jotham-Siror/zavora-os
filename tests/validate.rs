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
    assert!(!scenarios::intent_is_live("morning", true, false));
    assert!(scenarios::intent_is_live("deck", true, false));
    assert!(scenarios::intent_is_live("morning", false, true));
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