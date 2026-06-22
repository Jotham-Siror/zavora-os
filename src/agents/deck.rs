use std::sync::Arc;

use adk_agent::{ParallelAgent, SequentialAgent};

use super::{docs, excel, slides};

pub struct McpPool {
    pub worksheet: Arc<dyn adk_core::Toolset>,
    pub docx: Arc<dyn adk_core::Toolset>,
    pub slides: Arc<dyn adk_core::Toolset>,
}

pub async fn build_workflow(
    api_key: &str,
    model_name: &str,
    pool: &McpPool,
) -> anyhow::Result<Arc<dyn adk_core::Agent>> {
    let excel_agent = excel::build(api_key, model_name, pool.worksheet.clone()).await?;
    let docs_agent = docs::build(api_key, model_name, pool.docx.clone()).await?;
    let slides_agent = slides::build(api_key, model_name, pool.slides.clone()).await?;

    let parallel = ParallelAgent::new(
        "deck_parallel",
        vec![excel_agent, docs_agent],
    )
    .with_description("Excel + Docs in parallel")
    .with_shared_state();

    let pipeline = SequentialAgent::new(
        "deck_workflow",
        vec![
            Arc::new(parallel) as Arc<dyn adk_core::Agent>,
            slides_agent,
        ],
    )
    .with_description("Pitch deck: parallel excel+docs, then slides");

    Ok(Arc::new(pipeline))
}