//! Spawn MCP child processes with auto-reconnect (pattern from docx-agent-app).

use adk_core::Toolset;
use adk_tool::mcp::{ConnectionFactory, McpToolset, RefreshConfig};
use anyhow::Context;
use rmcp::{service::RunningService, transport::TokioChildProcess, RoleClient, ServiceExt};
use std::path::Path;
use std::sync::Arc;

pub const MAX_RESTART_ATTEMPTS: u32 = 3;

struct ProcessConnectionFactory {
    path: String,
}

#[async_trait::async_trait]
impl ConnectionFactory<()> for ProcessConnectionFactory {
    async fn create_connection(&self) -> Result<RunningService<RoleClient, ()>, String> {
        let child = TokioChildProcess::new(tokio::process::Command::new(&self.path))
            .map_err(|e| e.to_string())?;
        ().serve(child).await.map_err(|e| e.to_string())
    }
}

pub async fn spawn_mcp_server(path: &Path) -> anyhow::Result<McpToolset<()>> {
    anyhow::ensure!(
        path.exists(),
        "MCP server binary not found at {} — build it first",
        path.display()
    );

    let path_str = path.to_string_lossy().into_owned();
    tracing::info!("Spawning MCP server: {path_str}");

    let child = TokioChildProcess::new(tokio::process::Command::new(&path_str))
        .context("failed to spawn MCP child process")?;
    let client = ().serve(child).await.context("MCP handshake failed")?;

    let factory = Arc::new(ProcessConnectionFactory { path: path_str });
    let toolset = McpToolset::new(client)
        .with_connection_factory(factory)
        .with_refresh_config(RefreshConfig::default().with_max_attempts(MAX_RESTART_ATTEMPTS));

    Ok(toolset)
}

pub async fn health_check(toolset: &McpToolset<()>) -> anyhow::Result<usize> {
    let ctx: Arc<dyn adk_core::ReadonlyContext> =
        Arc::new(adk_tool::SimpleToolContext::new("health"));
    let tools = toolset.tools(ctx).await.context("MCP list tools failed")?;
    Ok(tools.len())
}