//! Collect SSE orchestration events from a response body into JSON (A2A aggregation).

use axum::body::Body;
use axum::response::Response;
use futures::StreamExt;

pub async fn collect_sse_events(response: Response) -> Vec<serde_json::Value> {
    let body: Body = response.into_body();
    let mut stream = http_body_util::BodyExt::into_data_stream(body);
    let mut buf = String::new();
    let mut events = Vec::new();

    while let Some(chunk) = stream.next().await {
        let Ok(bytes) = chunk else { break };
        buf.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(pos) = buf.find("\n\n") {
            let part = buf[..pos].to_string();
            buf = buf[pos + 2..].to_string();
            if let Some(line) = part.lines().find(|l| l.starts_with("data: ")) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line[6..]) {
                    events.push(value);
                }
            }
        }
    }

    events
}