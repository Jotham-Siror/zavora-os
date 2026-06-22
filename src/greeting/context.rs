use std::sync::Arc;

use adk_tool::SimpleToolContext;
use chrono::Timelike;
use serde::Serialize;

use crate::agents::morning::MorningMcpPool;

#[derive(Clone, Debug, Default, Serialize)]
pub struct CalendarFacts {
    pub meeting_count: usize,
    pub first_title: Option<String>,
    pub first_start: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct InboxFacts {
    pub needs_reply: usize,
    pub subjects: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct NewsFacts {
    pub headline: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct WeatherFacts {
    pub location: Option<String>,
    pub summary: Option<String>,
    pub temperature_f: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct GreetingSnapshot {
    pub salutation: String,
    pub calendar: Option<CalendarFacts>,
    pub inbox: Option<InboxFacts>,
    pub news: Option<NewsFacts>,
    pub weather: Option<WeatherFacts>,
    pub connected: Vec<String>,
}

impl GreetingSnapshot {
    pub fn has_integration_facts(&self) -> bool {
        self.calendar.is_some()
            || self.inbox.is_some()
            || self.news.is_some()
            || self.weather.is_some()
    }
}

pub fn salutation_for_hour(hour: u32) -> &'static str {
    if hour < 12 {
        "Good morning"
    } else if hour < 17 {
        "Good afternoon"
    } else {
        "Good evening"
    }
}

pub fn salutation_now() -> String {
    salutation_for_hour(chrono::Local::now().hour()).into()
}

pub async fn gather(pool: Option<&MorningMcpPool>) -> GreetingSnapshot {
    let mut snap = GreetingSnapshot {
        salutation: salutation_now(),
        ..Default::default()
    };

    let Some(pool) = pool else {
        return snap;
    };

    if let Some(calendar) = pool.calendar.clone() {
        if let Some(facts) = fetch_calendar(calendar).await {
            snap.connected.push("calendar".into());
            snap.calendar = Some(facts);
        }
    }

    if let Some(email) = pool.email.clone() {
        if let Some(facts) = fetch_inbox(email).await {
            if facts.needs_reply > 0 {
                snap.connected.push("email".into());
                snap.inbox = Some(facts);
            }
        }
    }

    if let Some(facts) = fetch_news(pool.news.clone()).await {
        snap.connected.push("news".into());
        snap.news = Some(facts);
    }

    if let Some(facts) = fetch_weather(pool.weather.clone()).await {
        snap.connected.push("weather".into());
        snap.weather = Some(facts);
    }

    snap
}

async fn exec_tool(
    toolset: Arc<dyn adk_core::Toolset>,
    tool_name: &str,
    args: serde_json::Value,
) -> Option<serde_json::Value> {
    let ctx: Arc<dyn adk_core::ReadonlyContext> =
        Arc::new(SimpleToolContext::new("greeting-context"));
    let tools = toolset.tools(ctx.clone()).await.ok()?;
    let tool = tools.iter().find(|t| t.name() == tool_name)?;
    tool.execute(
        Arc::new(SimpleToolContext::new("greeting-context")) as Arc<dyn adk_core::ToolContext>,
        args,
    )
    .await
    .ok()
}

fn tool_output_string(resp: &serde_json::Value) -> Option<&str> {
    resp.get("output").and_then(|o| o.as_str())
}

async fn fetch_calendar(toolset: Arc<dyn adk_core::Toolset>) -> Option<CalendarFacts> {
    let resp = exec_tool(
        toolset,
        "get_today",
        serde_json::json!({ "calendar_id": "primary" }),
    )
    .await?;
    let output = tool_output_string(&resp)?;
    let events = serde_json::from_str::<serde_json::Value>(output)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .or_else(|| resp.get("events").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default();

    let first = events.first();
    Some(CalendarFacts {
        meeting_count: events.len(),
        first_title: first
            .and_then(|e| e.get("summary").or_else(|| e.get("title")))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        first_start: first
            .and_then(|e| e.get("start"))
            .and_then(|s| {
                s.as_str()
                    .map(str::to_string)
                    .or_else(|| s.get("dateTime").and_then(|v| v.as_str()).map(str::to_string))
            }),
    })
}

async fn fetch_inbox(toolset: Arc<dyn adk_core::Toolset>) -> Option<InboxFacts> {
    let resp = exec_tool(toolset, "list_inbox", serde_json::json!({ "limit": 12 })).await?;
    let output = tool_output_string(&resp)?;
    let messages = serde_json::from_str::<serde_json::Value>(output)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .or_else(|| resp.get("messages").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default();

    let subjects: Vec<String> = messages
        .iter()
        .take(3)
        .filter_map(|m| m.get("subject").and_then(|s| s.as_str()).map(str::to_string))
        .collect();

    Some(InboxFacts {
        needs_reply: messages.len(),
        subjects,
    })
}

fn headline_from_articles(articles: &[serde_json::Value]) -> Option<NewsFacts> {
    let first = articles.first()?;
    let title = first.get("title").and_then(|v| v.as_str())?;
    if title.is_empty()
        || title.starts_with("Error")
        || title.contains("not configured")
        || title.contains("GNews not configured")
    {
        return None;
    }
    let source = first
        .get("source")
        .and_then(|s| s.as_str().or_else(|| s.get("name").and_then(|n| n.as_str())))
        .or_else(|| first.get("by").and_then(|v| v.as_str()))
        .map(str::to_string);
    Some(NewsFacts {
        headline: Some(title.to_string()),
        source,
    })
}

fn parse_articles_json(output: &str) -> Option<Vec<serde_json::Value>> {
    let value = serde_json::from_str::<serde_json::Value>(output).ok()?;
    if let Some(arr) = value.as_array() {
        return Some(arr.clone());
    }
    value
        .get("articles")
        .and_then(|a| a.as_array())
        .cloned()
}

async fn fetch_news(toolset: Arc<dyn adk_core::Toolset>) -> Option<NewsFacts> {
    if let Some(resp) = exec_tool(
        toolset.clone(),
        "gnews_top_headlines",
        serde_json::json!({ "country": "us", "limit": 3 }),
    )
    .await
    {
        if let Some(output) = tool_output_string(&resp) {
            if let Some(articles) = parse_articles_json(output) {
                if let Some(facts) = headline_from_articles(&articles) {
                    return Some(facts);
                }
            }
        }
    }

    let resp = exec_tool(
        toolset,
        "hn_stories",
        serde_json::json!({ "story_type": "top", "limit": 3 }),
    )
    .await?;
    let output = tool_output_string(&resp)?;
    let articles = parse_articles_json(output)?;
    headline_from_articles(&articles)
}

fn weather_code_label(code: i64) -> &'static str {
    match code {
        0 => "clear",
        1 | 2 | 3 => "partly cloudy",
        45 | 48 => "foggy",
        51 | 53 | 55 => "drizzle",
        61 | 63 | 65 => "rain",
        71 | 73 | 75 => "snow",
        80 | 81 | 82 => "showers",
        95 | 96 | 99 => "thunderstorms",
        _ => "variable",
    }
}

async fn fetch_weather(toolset: Arc<dyn adk_core::Toolset>) -> Option<WeatherFacts> {
    let geo = exec_tool(
        toolset.clone(),
        "geocode_location",
        serde_json::json!({ "name": "San Francisco" }),
    )
    .await?;
    let geo_out = tool_output_string(&geo)?;
    let geo_json: serde_json::Value = serde_json::from_str(geo_out).ok()?;
    let place = geo_json
        .get("results")
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())?;
    let lat = place.get("latitude")?.as_f64()?;
    let lon = place.get("longitude")?.as_f64()?;
    let location = place
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let resp = exec_tool(
        toolset,
        "get_current_weather",
        serde_json::json!({ "latitude": lat, "longitude": lon }),
    )
    .await?;
    let output = tool_output_string(&resp)?;
    let wx: serde_json::Value = serde_json::from_str(output).ok()?;
    let current = wx.get("current")?;
    let temp = current.get("temperature_2m").and_then(|v| v.as_f64());
    let summary = current
        .get("weather_code")
        .and_then(|v| v.as_i64())
        .map(weather_code_label)
        .map(str::to_string);

    if temp.is_none() && summary.is_none() {
        return None;
    }

    Some(WeatherFacts {
        location,
        summary,
        temperature_f: temp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salutation_varies_by_hour() {
        assert_eq!(salutation_for_hour(8), "Good morning");
        assert_eq!(salutation_for_hour(14), "Good afternoon");
        assert_eq!(salutation_for_hour(21), "Good evening");
    }
}