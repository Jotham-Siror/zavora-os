use axum::response::sse::Event;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldEvent {
    Scenario {
        key: String,
        text: String,
        total_cards: usize,
    },
    CardSpawn {
        index: usize,
        card: serde_json::Value,
    },
    CardStatus {
        index: usize,
        status: String,
        line: Option<String>,
    },
    CardResolve {
        index: usize,
        resolve: serde_json::Value,
    },
    CardSurface {
        index: usize,
        surface: String,
        slide: u32,
        total: u32,
    },
    Error {
        message: String,
    },
    SuzySummary {
        key: String,
        html: String,
    },
    Done,
}

pub fn to_event(ev: &FieldEvent) -> Event {
    Event::default().data(serde_json::to_string(ev).unwrap_or_else(|_| "{}".into()))
}