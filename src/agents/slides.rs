use std::sync::Arc;

use adk_agent::LlmAgentBuilder;
use adk_model::gemini::GeminiModel;

use super::gemini;

const SLIDES_TOOLS: &[&str] = &[
    "create_presentation",
    "add_slide",
    "save_presentation",
    "describe_presentation",
    "list_templates",
    "render_slide",
];

pub async fn build(
    api_key: &str,
    model_name: &str,
    toolset: Arc<dyn adk_core::Toolset>,
) -> anyhow::Result<Arc<dyn adk_core::Agent>> {
    let model = Arc::new(GeminiModel::new(api_key, model_name)?);
    let tools = gemini::filtered(gemini::wrap_toolset(toolset), SLIDES_TOOLS);

    let agent = LlmAgentBuilder::new("slides_agent")
        .description("Builds the pitch deck presentation")
        .model(model)
        .instruction(
            r#"You are Auto-Slides for a pitch deck. Build a ~10 slide .pptx.

Rules:
- Wait for sibling artifacts: read paths listed under [Sibling artifacts] in the user message.
- create_presentation → add_slide for each slide → save_presentation.
- Filename: pitch_deck.pptx in the [Save files to: ...] directory.
- Pull numbers from the .xlsx context and narrative from the .docx context.
- Use add_slide once per slide so the UI filmstrip can update.
- Minimize narration."#,
        )
        .toolset(tools)
        .tool_execution_strategy(adk_core::ToolExecutionStrategy::Parallel)
        .generate_content_config(adk_core::GenerateContentConfig {
            temperature: Some(0.2),
            max_output_tokens: Some(32768),
            ..Default::default()
        })
        .build()?;

    Ok(Arc::new(agent))
}