use std::sync::Arc;

/// Merges tool lists from multiple MCP toolsets (e.g. news + weather for brief_agent).
pub struct MergedToolset {
    parts: Vec<Arc<dyn adk_core::Toolset>>,
}

impl MergedToolset {
    pub fn new(parts: Vec<Arc<dyn adk_core::Toolset>>) -> Arc<Self> {
        Arc::new(Self { parts })
    }
}

#[async_trait::async_trait]
impl adk_core::Toolset for MergedToolset {
    fn name(&self) -> &str {
        "merged"
    }

    async fn tools(
        &self,
        ctx: Arc<dyn adk_core::ReadonlyContext>,
    ) -> adk_core::Result<Vec<Arc<dyn adk_core::Tool>>> {
        let mut all = Vec::new();
        for part in &self.parts {
            all.extend(part.tools(ctx.clone()).await?);
        }
        Ok(all)
    }
}