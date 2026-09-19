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
        "You are Suzy — warm, confident, quietly witty voice of the Mother Agent in Zavora Personal AI OS. \
         Help the user express intent, start their day, and let the Mother Agent orchestrate the specialized agents. \
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
    let sid_for_context = session_id.clone();
    let sid_for_intent = session_id.clone();
    let state_for_intent = state.clone();

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
                let sid = sid_for_context.clone();
                if let (Some(sid), Ok(handle)) = (sid, tokio::runtime::Handle::try_current()) {
                    if let Some(record) = handle.block_on(sessions.get(&sid)) {
                        return Ok(json!({
                            "session_id": record.session_id,
                            "context": suzy::session_context(&record),
                            "domains": crate::mother::domain_summary(&record),
                        }));
                    }
                }
                Ok(json!({
                    "session_id": null,
                    "context": "No active session — ask what the user would like to do."
                }))
            }),
        )
        .tool(
            ToolDefinition {
                name: "submit_intent".into(),
                description: Some(
                    "Start orchestration for a user request (deck, morning brief, trip, etc.). \
                     Call when the user asks to do something that should spawn field cards."
                        .into(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The user's intent in natural language"
                        }
                    },
                    "required": ["text"]
                })),
            },
            FnToolHandler::new(move |call| {
                let text = call.arguments["text"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if text.is_empty() {
                    return Ok(json!({
                        "status": "error",
                        "message": "intent text required"
                    }));
                }

                let state = state_for_intent.clone();
                let sid = sid_for_intent.clone();
                let handle = tokio::runtime::Handle::try_current().ok();
                let Some(handle) = handle else {
                    return Ok(json!({
                        "status": "error",
                        "message": "runtime unavailable"
                    }));
                };

                let (session_id, user_id) = match sid {
                    Some(ref id) => {
                        let Some(record) = handle.block_on(state.sessions.get(id)) else {
                            return Ok(json!({
                                "status": "error",
                                "message": "session not found"
                            }));
                        };
                        (record.session_id, record.user_id)
                    }
                    None => {
                        let record = handle.block_on(state.sessions.create());
                        (record.session_id, record.user_id)
                    }
                };

                Ok(json!({
                    "status": "started",
                    "session_id": session_id,
                    "user_id": user_id,
                    "intent": text,
                    "dispatch": "client_sse"
                }))
            }),
        )
        .build()?;

    Ok(runner)
}