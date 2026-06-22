use std::sync::Arc;

use adk_agent::LlmAgentBuilder;
use adk_model::gemini::GeminiModel;

use super::gemini;
use crate::tools::merge::MergedToolset;

const BRIEF_TOOLS: &[&str] = &[
    "gnews_top_headlines",
    "search_news",
    "get_country_news",
    "get_forecast",
    "geocode_location",
];

pub async fn build(
    api_key: &str,
    model_name: &str,
    news: Arc<dyn adk_core::Toolset>,
    weather: Arc<dyn adk_core::Toolset>,
) -> anyhow::Result<Arc<dyn adk_core::Agent>> {
    let model = Arc::new(GeminiModel::new(api_key, model_name)?);
    let merged = MergedToolset::new(vec![news, weather]);
    let tools = gemini::filtered(gemini::wrap_toolset(merged), BRIEF_TOOLS);

    let agent = LlmAgentBuilder::new("brief_agent")
        .description("Composes the morning brief from news and weather")
        .model(model)
        .instruction(
            r#"You are the Brief card for a morning briefing.

Rules:
- Read sibling outputs from shared state (calendar + inbox summaries) if present.
- gnews_top_headlines or search_news for 2–3 headlines (country us unless user specifies).
- get_forecast for user's location (default San Francisco if unknown; geocode_location first if needed).
- Output 3 brief lines: top headline, weather/commute note, one actionable insight from calendar/inbox context.
- Minimize narration — use tools first."#,
        )
        .toolset(tools)
        .tool_execution_strategy(adk_core::ToolExecutionStrategy::Parallel)
        .generate_content_config(adk_core::GenerateContentConfig {
            temperature: Some(0.3),
            max_output_tokens: Some(8192),
            ..Default::default()
        })
        .build()?;

    Ok(Arc::new(agent))
}