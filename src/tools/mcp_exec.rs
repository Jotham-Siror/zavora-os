//! Shared MCP tool invocation for greeting and rails.

use std::sync::Arc;

use adk_tool::SimpleToolContext;

pub async fn exec_tool(
    toolset: Arc<dyn adk_core::Toolset>,
    tool_name: &str,
    args: serde_json::Value,
) -> Option<serde_json::Value> {
    let ctx: Arc<dyn adk_core::ReadonlyContext> = Arc::new(SimpleToolContext::new("mcp-exec"));
    let tools = toolset.tools(ctx.clone()).await.ok()?;
    let tool = tools.iter().find(|t| t.name() == tool_name)?;
    tool.execute(
        Arc::new(SimpleToolContext::new("mcp-exec")) as Arc<dyn adk_core::ToolContext>,
        args,
    )
    .await
    .ok()
}

pub fn tool_output_string(resp: &serde_json::Value) -> Option<&str> {
    resp.get("output").and_then(|o| o.as_str())
}

pub fn parse_json_output(output: &str) -> Option<serde_json::Value> {
    serde_json::from_str(output).ok()
}

pub fn parse_array_output(output: &str) -> Option<Vec<serde_json::Value>> {
    let value = parse_json_output(output)?;
    if let Some(arr) = value.as_array() {
        return Some(arr.clone());
    }
    value
        .get("articles")
        .or_else(|| value.get("members"))
        .or_else(|| value.get("users"))
        .and_then(|a| a.as_array())
        .cloned()
}