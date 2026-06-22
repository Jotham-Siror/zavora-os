use std::sync::Arc;

use adk_agent::{ParallelAgent, SequentialAgent};

use super::{brief, calendar, inbox};

pub struct MorningMcpPool {
    pub calendar: Option<Arc<dyn adk_core::Toolset>>,
    pub email: Option<Arc<dyn adk_core::Toolset>>,
    pub news: Arc<dyn adk_core::Toolset>,
    pub weather: Arc<dyn adk_core::Toolset>,
}

pub async fn build_workflow(
    api_key: &str,
    model_name: &str,
    pool: &MorningMcpPool,
) -> anyhow::Result<Arc<dyn adk_core::Agent>> {
    let mut parallel: Vec<Arc<dyn adk_core::Agent>> = Vec::new();

    if let Some(calendar_ts) = pool.calendar.clone() {
        parallel.push(calendar::build(api_key, model_name, calendar_ts).await?);
    }
    if let Some(email_ts) = pool.email.clone() {
        parallel.push(inbox::build(api_key, model_name, email_ts).await?);
    }

    let brief_agent = brief::build(
        api_key,
        model_name,
        pool.news.clone(),
        pool.weather.clone(),
    )
    .await?;

    if parallel.is_empty() {
        return Ok(Arc::new(
            SequentialAgent::new("morning_workflow", vec![brief_agent])
                .with_description("Morning brief (news + weather)"),
        ));
    }

    let parallel = ParallelAgent::new("morning_parallel", parallel)
        .with_description("Calendar + inbox in parallel")
        .with_shared_state();

    let pipeline = SequentialAgent::new(
        "morning_workflow",
        vec![
            Arc::new(parallel) as Arc<dyn adk_core::Agent>,
            brief_agent,
        ],
    )
    .with_description("Morning: parallel calendar+inbox, then brief");

    Ok(Arc::new(pipeline))
}