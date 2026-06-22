use std::sync::Arc;

use adk_runner::Runner;
use tokio_stream::wrappers::ReceiverStream;

use crate::orchestrator::workflow::{AgentSlot, WorkflowStreamConfig};
use crate::orchestrator::workflow::stream_workflow;
use crate::state::SessionStore;

fn live_cards() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "glyph":"📰","title":"Headlines","agent":"news.agent",
            "stream":["Scanning live sources…","Filtering to what you follow…"]
        }),
        serde_json::json!({
            "glyph":"📈","title":"Markets","agent":"markets.agent",
            "stream":["Checking your watchlist…"]
        }),
        serde_json::json!({
            "glyph":"🔴","title":"Now","agent":"live.agent","waitsFor":2,
            "stream":["Tuning into live events…"]
        }),
    ]
}

fn status_line(tool: &str) -> &'static str {
    match tool {
        "gnews_top_headlines" | "search_news" | "get_country_news" => "Scanning live sources…",
        "get_trending_topics" => "Filtering to what you follow…",
        "yfinance_chart" => "Checking your watchlist…",
        _ => "Working…",
    }
}

fn headlines_resolve(text: &str) -> serde_json::Value {
    let count = text.lines().filter(|l| !l.trim().is_empty()).count().max(1);
    let sub = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("Top stories from live sources")
        .chars()
        .take(80)
        .collect::<String>();
    serde_json::json!({
        "big": format!("{count} big stories"),
        "sub": sub,
        "actions": ["Read aloud", "Open"]
    })
}

fn markets_resolve(text: &str) -> serde_json::Value {
    let lower = text.to_lowercase();
    let pct = if lower.contains('+') {
        "+0.6% pre-open"
    } else if lower.contains('-') {
        "−0.4% pre-open"
    } else {
        "Markets mixed"
    };
    serde_json::json!({
        "big": pct,
        "sub": text.lines().next().unwrap_or("Watchlist update").chars().take(60).collect::<String>(),
        "actions": ["Details", "Set alert"]
    })
}

fn now_resolve(text: &str) -> serde_json::Value {
    let lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(3)
        .map(|l| {
            if l.starts_with("<b>") || l.starts_with('🔴') {
                l.to_string()
            } else {
                format!("<b>•</b> {l}")
            }
        })
        .collect();
    let lines = if lines.is_empty() {
        vec![
            "<b>•</b> Trending topics loading".into(),
            "<b>•</b> Markets in focus".into(),
            "<b>•</b> Check headlines card".into(),
        ]
    } else {
        lines
    };
    serde_json::json!({
        "lines": lines,
        "actions": ["Watch", "Dismiss"]
    })
}

fn resolve(author: &str, tool_text: &str, agent_text: &str) -> serde_json::Value {
    let combined = if agent_text.is_empty() {
        tool_text.to_string()
    } else {
        format!("{tool_text}\n{agent_text}")
    };
    match author {
        "headlines_agent" => headlines_resolve(&combined),
        "markets_agent" => markets_resolve(&combined),
        "now_agent" => now_resolve(&combined),
        _ => serde_json::json!({ "big": "Ready", "sub": "Done", "actions": ["Open"] }),
    }
}

pub fn stream_live(
    runner: Arc<Runner>,
    user_id: String,
    session_id: String,
    intent: String,
    has_market_data: bool,
    sessions: Option<SessionStore>,
    suzy_runner: Option<Arc<Runner>>,
) -> ReceiverStream<Result<axum::response::sse::Event, std::convert::Infallible>> {
    stream_workflow(
        runner,
        user_id,
        session_id,
        intent,
        WorkflowStreamConfig {
            scenario_key: "live",
            cards: live_cards(),
            slots: vec![
                AgentSlot {
                    author: "headlines_agent",
                    index: 0,
                },
                AgentSlot {
                    author: "markets_agent",
                    index: 1,
                },
                AgentSlot {
                    author: "now_agent",
                    index: 2,
                },
            ],
            status_line,
            resolve,
            integrations_note: format!(
                "news: live\nmarket_data: {}",
                if has_market_data {
                    "live"
                } else {
                    "via mcp-news yfinance"
                }
            ),
        },
        sessions,
        suzy_runner,
    )
}