//! Integration test: deck intent produces .xlsx, .docx, .pptx (BK-004).
//! Run: `GOOGLE_API_KEY=... cargo test --test deck_workflow -- --ignored`

use std::path::PathBuf;
use std::sync::Arc;

use adk_core::{Content, SessionId, UserId};
use adk_runner::Runner;
use adk_session::{CreateRequest, InMemorySessionService, SessionService};
use futures::StreamExt;

#[tokio::test]
#[ignore = "requires GOOGLE_API_KEY and built MCP server binaries"]
async fn deck_workflow_writes_three_artifacts() {
    dotenvy::dotenv().ok();
    let api_key = std::env::var("GOOGLE_API_KEY").expect("GOOGLE_API_KEY");

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let worksheet = manifest.join("../mcp-servers/worksheet-mcp/target/release/excel-mcp-server");
    let docx = manifest.join("../mcp-servers/docx-mcp/target/release/docx-mcp-server");
    let slides = manifest.join("../mcp-servers/mcp_slides/target/release/slides-mcp-server");

    let worksheet_ts = spatial_os::tools::mcp::spawn_mcp_server(&worksheet)
        .await
        .expect("worksheet mcp");
    let docx_ts = spatial_os::tools::mcp::spawn_mcp_server(&docx).await.expect("docx mcp");
    let slides_ts = spatial_os::tools::mcp::spawn_mcp_server(&slides)
        .await
        .expect("slides mcp");

    let pool = spatial_os::agents::deck::McpPool {
        worksheet: Arc::new(worksheet_ts),
        docx: Arc::new(docx_ts),
        slides: Arc::new(slides_ts),
    };

    let workflow = spatial_os::agents::deck::build_workflow(
        &api_key,
        "gemini-2.5-flash",
        &pool,
    )
    .await
    .expect("workflow");

    let session_service = Arc::new(InMemorySessionService::new());
    let runner = Runner::builder()
        .app_name("zavora-os-test")
        .agent(workflow)
        .session_service(session_service.clone())
        .build()
        .expect("runner");

    let session_id = uuid::Uuid::new_v4().to_string();
    let user_id = uuid::Uuid::new_v4().to_string();
    session_service
        .create(CreateRequest {
            app_name: "zavora-os-test".into(),
            user_id: user_id.clone(),
            session_id: Some(session_id.clone()),
            state: Default::default(),
        })
        .await
        .expect("session");

    let artifact_root = tempfile::tempdir().expect("tmpdir");
    let session_dir = artifact_root.path().join(&session_id);
    std::fs::create_dir_all(&session_dir).unwrap();

    let prompt = format!(
        "Build me a pitch deck\n\n[Save files to: {}/]\n\
         [Sibling artifacts]\n(none yet)\n",
        session_dir.display()
    );

    let content = Content::new("user").with_text(&prompt);
    let uid = UserId::try_from(user_id.as_str()).unwrap();
    let sid = SessionId::try_from(session_id.as_str()).unwrap();

    let mut stream = runner.run(uid, sid, content).await.expect("run");
    while let Some(ev) = stream.next().await {
        ev.expect("event");
    }

    for ext in ["xlsx", "docx", "pptx"] {
        let found = std::fs::read_dir(&session_dir)
            .expect("read dir")
            .flatten()
            .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some(ext));
        assert!(found, "expected .{ext} in {}", session_dir.display());
    }
}