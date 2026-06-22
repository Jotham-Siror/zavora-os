use std::sync::Arc;

use adk_agent::LlmAgentBuilder;
use adk_model::gemini::GeminiModel;

use super::gemini;

const EXCEL_TOOLS: &[&str] = &[
    "create_workbook",
    "open_workbook",
    "save_workbook",
    "write_cells",
    "write_column",
    "write_row",
    "add_chart",
    "describe_workbook",
    "list_sheets",
    "set_cell_format",
];

pub async fn build(
    api_key: &str,
    model_name: &str,
    toolset: Arc<dyn adk_core::Toolset>,
) -> anyhow::Result<Arc<dyn adk_core::Agent>> {
    let model = Arc::new(GeminiModel::new(api_key, model_name)?);
    let tools = gemini::filtered(gemini::wrap_toolset(toolset), EXCEL_TOOLS);

    let agent = LlmAgentBuilder::new("excel_agent")
        .description("Builds the pitch deck revenue spreadsheet")
        .model(model)
        .instruction(
            r#"You are Auto-Excel for a pitch deck. Build a concise revenue model spreadsheet.

Rules:
- Save to the [Save files to: ...] directory from the user message.
- Use create_workbook → save_workbook → open_workbook, then write data, then save again.
- Filename: pitch_model.xlsx
- Include at least: revenue headline, QoQ growth, one chart.
- Call save_workbook after every batch of writes so progress is visible.
- Minimize narration — act with tools."#,
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