use std::sync::Arc;

use adk_agent::LlmAgentBuilder;
use adk_model::gemini::GeminiModel;

use crate::agents::gemini;
use crate::agents::stub;

const REAL_ESTATE_TOOLS: &[&str] = &["search_properties_nearby", "geocode_search"];

pub async fn build(
    api_key: &str,
    model_name: &str,
    real_estate: Option<Arc<dyn adk_core::Toolset>>,
) -> anyhow::Result<Arc<dyn adk_core::Agent>> {
    if let Some(ts) = real_estate {
        let model = Arc::new(GeminiModel::new(api_key, model_name)?);
        let tools = gemini::filtered(gemini::wrap_toolset(ts), REAL_ESTATE_TOOLS);
        let agent = LlmAgentBuilder::new("scout_agent")
            .description("Background price and listing scout")
            .model(model)
            .instruction(
                r#"You are a background scout agent.
Check property listings or market indicators for notable price drops.
Summarize one alert-worthy finding in 2-3 sentences."#,
            )
            .toolset(tools)
            .generate_content_config(adk_core::GenerateContentConfig {
                temperature: Some(0.25),
                max_output_tokens: Some(1024),
                ..Default::default()
            })
            .build()?;
        return Ok(Arc::new(agent));
    }

    stub::labeled_stub(
        "scout_agent",
        "STUB — connect mcp-real-estate",
        "Price drop alert: target listing fell 12% overnight. Connect real-estate MCP for live scout.",
    )
}