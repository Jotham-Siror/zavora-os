//! Labeled stub agents for MCP gaps (BK-009, BK-010).

use std::sync::Arc;

use adk_agent::CustomAgentBuilder;
use adk_core::{Content, Event};
use futures::stream;

pub fn labeled_stub(
    name: &'static str,
    label: &str,
    body: &str,
) -> anyhow::Result<Arc<dyn adk_core::Agent>> {
    let label = label.to_string();
    let body = body.to_string();
    let agent = CustomAgentBuilder::new(name)
        .description(format!("{label} — stub until MCP connected"))
        .handler(move |_ctx| {
            let label = label.clone();
            let body = body.clone();
            async move {
                let text = format!("[{label}] {body}");
                let mut event = Event::new("stub");
                event.author = name.to_string();
                event.llm_response.content = Some(Content::new("assistant").with_text(text));
                Ok(Box::pin(stream::iter(vec![Ok(event)])) as adk_core::EventStream)
            }
        })
        .build()?;
    Ok(Arc::new(agent))
}