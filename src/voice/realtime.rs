//! Suzy voice runner — Gemini Live via adk-realtime (mia pattern).

use adk_realtime::config::{RealtimeConfig, ToolDefinition};
use adk_realtime::runner::{FnToolHandler, RealtimeRunner};
use serde_json::json;

use crate::agents::suzy;
use crate::state::AppState;

pub async fn build_suzy_runner(
    state: &AppState,
    session_id: Option<String>,
) -> anyhow::Result<RealtimeRunner> {
    let model = state
        .voice
        .model
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Gemini Live not configured (set GOOGLE_API_KEY)"))?;

    let mut instruction = String::from(
        "You are Suzy — warm, confident, quietly witty voice of Zavora OS. \
         Help the user express intent, start their day, and orchestrate agents. \
         Keep replies concise and spoken-friendly (1–3 sentences unless they ask for detail).",
    );
    if let Some(tone) = &state.brand_tone {
        instruction.push_str(&format!("\nBrand tone: {tone}."));
    }
    instruction.push_str(&format!(
        "\nDefault greeting line when relevant: {}",
        state.brand_greeting_body
    ));
    if let Some(sid) = &session_id {
        instruction.push_str(&format!("\nActive UI session id: {sid}."));
        if let Some(record) = state.sessions.get(sid).await {
            instruction.push_str(&format!(
                "\n\nCurrent session context:\n{}",
                suzy::session_context(&record)
            ));
        }
    }

    let sessions = state.sessions.clone();
    let sid_for_tool = session_id.clone();

    let runner = RealtimeRunner::builder()
        .model(model)
        .config(
            RealtimeConfig::default()
                .with_instruction(&instruction)
                .with_voice(&state.voice.voice_name),
        )
        .tool(
            ToolDefinition {
                name: "get_session_context".into(),
                description: Some(
                    "Read the current browser session: scenario, cards, and artifacts.".into(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                })),
            },
            FnToolHandler::new(move |_call| {
                let sessions = sessions.clone();
                let sid = sid_for_tool.clone();
                if let (Some(sid), Ok(handle)) = (sid, tokio::runtime::Handle::try_current()) {
                    if let Some(record) = handle.block_on(sessions.get(&sid)) {
                        return Ok(json!({
                            "session_id": record.session_id,
                            "context": suzy::session_context(&record),
                        }));
                    }
                }
                Ok(json!({
                    "session_id": null,
                    "context": "No active session — ask what the user would like to do."
                }))
            }),
        )
        .build()?;

    Ok(runner)
}