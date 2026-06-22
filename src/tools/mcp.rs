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
    workdir: Option<String>,
}

#[async_trait::async_trait]
impl ConnectionFactory<()> for ProcessConnectionFactory {
    async fn create_connection(&self) -> Result<RunningService<RoleClient, ()>, String> {
        let mut cmd = tokio::process::Command::new(&self.path);
        if let Some(dir) = &self.workdir {
            cmd.current_dir(dir);
        }
        let child = TokioChildProcess::new(cmd).map_err(|e| e.to_string())?;
        ().serve(child).await.map_err(|e| e.to_string())
    }
}

fn mcp_package_root(binary: &Path) -> Option<std::path::PathBuf> {
    // .../mcp-news/target/release/mcp-news → .../mcp-news
    binary.parent()?.parent()?.parent().map(std::path::Path::to_path_buf)
}

pub async fn try_spawn_mcp_server(path: &Path) -> Option<McpToolset<()>> {
    match spawn_mcp_server(path).await {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::warn!("MCP unavailable at {} ({e:#})", path.display());
            None
        }
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

    let mut cmd = tokio::process::Command::new(&path_str);
    if let Some(pkg_root) = mcp_package_root(path) {
        if pkg_root.join("mcp-server.toml").exists() {
            cmd.current_dir(&pkg_root);
        }
    }

    let child = TokioChildProcess::new(cmd).context("failed to spawn MCP child process")?;
    let client = ().serve(child).await.context("MCP handshake failed")?;

    let workdir = mcp_package_root(path).filter(|d| d.join("mcp-server.toml").exists());
    let factory = Arc::new(ProcessConnectionFactory {
        path: path_str,
        workdir: workdir.map(|p| p.to_string_lossy().into_owned()),
    });
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