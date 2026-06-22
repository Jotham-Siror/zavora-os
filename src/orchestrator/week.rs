use std::sync::Arc;

use adk_runner::Runner;
use tokio_stream::wrappers::ReceiverStream;

use crate::orchestrator::workflow::{stream_workflow, AgentSlot, WorkflowStreamConfig};
use crate::state::SessionStore;

fn week_cards() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "glyph":"💸","title":"Money","agent":"finance.agent",
            "stream":["Reconciling accounts…"]
        }),
        serde_json::json!({
            "glyph":"💪","title":"Health","agent":"health.agent",
            "stream":["Reading activity & sleep…"]
        }),
        serde_json::json!({
            "glyph":"🧠","title":"Focus","agent":"work.agent","waitsFor":2,
            "stream":["Looking across your week…"]
        }),
    ]
}

fn status_line(_tool: &str) -> &'static str {
    "Working…"
}

fn money_resolve(text: &str) -> serde_json::Value {
    let stub = text.contains("STUB");
    serde_json::json!({
        "big": if stub { "−$1,240 (stub)" } else { "Weekly spend" },
        "sub": text.lines().next().unwrap_or("vs last week").chars().take(60).collect::<String>(),
        "actions": ["Breakdown", "Set limit"]
    })
}

fn health_resolve(text: &str) -> serde_json::Value {
    serde_json::json!({
        "big": if text.contains("avg sleep") {
            text.split("avg sleep").nth(1).unwrap_or("6.1h").trim().chars().take(20).collect::<String>()
        } else {
            "6.1h avg sleep".into()
        },
        "sub": if text.contains("STUB") { "STUB — set HEALTH_CSV_PATH" } else { "Imported health data" },
        "actions": ["Tips", "Plan rest"]
    })
}

fn focus_resolve(text: &str) -> serde_json::Value {
    let lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(3)
        .map(|l| format!("<b>•</b> {l}"))
        .collect();
    let lines = if lines.is_empty() {
        vec![
            "<b>•</b> Shipped 14 commits, 2 features".into(),
            "<b>•</b> Most focused: Tue mornings".into(),
            "<b>•</b> Suggest: protect Tue 9–12".into(),
        ]
    } else {
        lines
    };
    serde_json::json!({
        "lines": lines,
        "actions": ["Apply", "Ignore"]
    })
}

fn resolve(author: &str, tool_text: &str, agent_text: &str) -> serde_json::Value {
    let combined = if agent_text.is_empty() {
        tool_text.to_string()
    } else {
        format!("{tool_text}\n{agent_text}")
    };
    match author {
        "money_agent" => money_resolve(&combined),
        "health_agent" => health_resolve(&combined),
        "focus_agent" => focus_resolve(&combined),
        _ => serde_json::json!({ "big": "Ready", "sub": "Done", "actions": ["Open"] }),
    }
}

pub fn stream_week(
    runner: Arc<Runner>,
    user_id: String,
    session_id: String,
    intent: String,
    has_banking: bool,
    has_github: bool,
    has_health_csv: bool,
    sessions: Option<SessionStore>,
    suzy_runner: Option<Arc<Runner>>,
) -> ReceiverStream<Result<axum::response::sse::Event, std::convert::Infallible>> {
    stream_workflow(
        runner,
        user_id,
        session_id,
        intent,
        WorkflowStreamConfig {
            scenario_key: "week",
            cards: week_cards(),
            slots: vec![
                AgentSlot {
                    author: "money_agent",
                    index: 0,
                },
                AgentSlot {
                    author: "health_agent",
                    index: 1,
                },
                AgentSlot {
                    author: "focus_agent",
                    index: 2,
                },
            ],
            status_line,
            resolve,
            integrations_note: format!(
                "banking: {}\ngithub: {}\nhealth_csv: {}",
                if has_banking { "live" } else { "stub" },
                if has_github { "live" } else { "stub" },
                if has_health_csv { "live" } else { "stub" },
            ),
        },
        sessions,
        suzy_runner,
    )
}