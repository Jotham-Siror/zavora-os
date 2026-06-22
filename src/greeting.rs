use std::sync::Arc;

use adk_tool::SimpleToolContext;

use crate::agents::morning::MorningMcpPool;

const DEFAULT_GREETING: &str =
    "Good morning. Here's your day — meetings, emails, and a free window at noon.";

pub async fn personalized_text(pool: Option<&MorningMcpPool>) -> (String, &'static str) {
    let Some(pool) = pool else {
        return (DEFAULT_GREETING.into(), "static");
    };

    let Some(calendar) = pool.calendar.clone() else {
        return (DEFAULT_GREETING.into(), "static");
    };

    match calendar_summary(calendar).await {
        Some(text) => (text, "calendar"),
        None => (DEFAULT_GREETING.into(), "static"),
    }
}

async fn calendar_summary(calendar: Arc<dyn adk_core::Toolset>) -> Option<String> {
    let ctx: Arc<dyn adk_core::ReadonlyContext> =
        Arc::new(SimpleToolContext::new("greeting"));
    let tools = calendar.tools(ctx.clone()).await.ok()?;
    let tool = tools.iter().find(|t| t.name() == "get_today")?;
    let resp = tool
        .execute(
            Arc::new(SimpleToolContext::new("greeting")) as Arc<dyn adk_core::ToolContext>,
            serde_json::json!({ "calendar_id": "primary" }),
        )
        .await
        .ok()?;

    let output = resp
        .get("output")
        .and_then(|o| o.as_str())
        .unwrap_or("");
    let count = serde_json::from_str::<serde_json::Value>(output)
        .ok()
        .and_then(|v| v.as_array().map(|a| a.len()))
        .unwrap_or(0);

    if count == 0 {
        Some("Good morning. Your calendar is clear today — a good day to focus.".into())
    } else if count == 1 {
        Some("Good morning. You have one meeting today — and time to breathe between.".into())
    } else {
        Some(format!(
            "Good morning. You have {count} meetings today — I've lined up your brief."
        ))
    }
}