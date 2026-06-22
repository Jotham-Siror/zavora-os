use std::sync::Arc;

use serde::Serialize;

use crate::tools::mcp_exec;

#[derive(Clone, Debug, Serialize)]
pub struct LiveSlide {
    pub cls: String,
    pub logo: String,
    pub name: String,
    pub when: String,
    pub body: String,
    pub meta: String,
}

const SLIDE_STYLES: &[(&str, &str, &str)] = &[
    ("lf-hn", "H", "Hacker News"),
    ("lf-news", "N", "News"),
    ("lf-trend", "T", "Trending"),
    ("lf-market", "M", "Markets"),
];

pub async fn fetch_slides(news: Arc<dyn adk_core::Toolset>) -> (String, Vec<LiveSlide>) {
    let mut slides = Vec::new();

    if let Some(hn) = fetch_hn_slides(news.clone()).await {
        slides.extend(hn);
    }

    if slides.len() < 4 {
        if let Some(headlines) = fetch_gnews_slides(news.clone()).await {
            for s in headlines {
                if slides.len() >= 4 {
                    break;
                }
                if !slides.iter().any(|x| x.body == s.body) {
                    slides.push(s);
                }
            }
        }
    }

    if slides.is_empty() {
        return ("unavailable".into(), slides);
    }

    ("mcp-news".into(), slides)
}

async fn fetch_hn_slides(news: Arc<dyn adk_core::Toolset>) -> Option<Vec<LiveSlide>> {
    let resp = mcp_exec::exec_tool(
        news,
        "hn_stories",
        serde_json::json!({ "story_type": "top", "limit": 4 }),
    )
    .await?;
    let output = mcp_exec::tool_output_string(&resp)?;
    let stories = mcp_exec::parse_array_output(output)?;

    let mut slides = Vec::new();
    for (i, story) in stories.iter().take(4).enumerate() {
        let title = story.get("title").and_then(|v| v.as_str())?;
        if title.is_empty() || title.starts_with("Error") {
            continue;
        }
        let score = story
            .get("score")
            .and_then(|v| v.as_u64())
            .map(|s| format!("{s} pts"))
            .unwrap_or_else(|| "top story".into());
        let by = story
            .get("by")
            .and_then(|v| v.as_str())
            .map(|b| format!("by {b}"))
            .unwrap_or_default();
        let (cls, logo, name) = SLIDE_STYLES[i % SLIDE_STYLES.len()];
        slides.push(LiveSlide {
            cls: cls.into(),
            logo: logo.into(),
            name: name.into(),
            when: if i == 0 { "now".into() } else { format!("{}m", i * 3) },
            body: title.to_string(),
            meta: if by.is_empty() { score } else { format!("{score} · {by}") },
        });
    }

    if slides.is_empty() {
        None
    } else {
        Some(slides)
    }
}

async fn fetch_gnews_slides(news: Arc<dyn adk_core::Toolset>) -> Option<Vec<LiveSlide>> {
    let resp = mcp_exec::exec_tool(
        news,
        "gnews_top_headlines",
        serde_json::json!({ "country": "us", "limit": 3 }),
    )
    .await?;
    let output = mcp_exec::tool_output_string(&resp)?;
    if output.contains("not configured") {
        return None;
    }
    let articles = mcp_exec::parse_array_output(output)?;
    let mut slides = Vec::new();
    for (i, article) in articles.iter().take(3).enumerate() {
        let title = article.get("title").and_then(|v| v.as_str())?;
        let source = article
            .get("source")
            .and_then(|s| s.as_str().or_else(|| s.get("name").and_then(|n| n.as_str())))
            .unwrap_or("News");
        slides.push(LiveSlide {
            cls: "lf-news".into(),
            logo: source.chars().next().unwrap_or('N').to_uppercase().to_string(),
            name: source.to_string(),
            when: if i == 0 { "now".into() } else { format!("{}m", i * 5) },
            body: title.to_string(),
            meta: "Headlines".into(),
        });
    }
    if slides.is_empty() {
        None
    } else {
        Some(slides)
    }
}