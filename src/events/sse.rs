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
        #[serde(skip_serializing_if = "Option::is_none")]
        audio_clip: Option<String>,
    },
    Suggest {
        text: String,
        kind: String,
    },
    Conduct {
        steps: Vec<ConductStep>,
    },
    DeckFinish {
        big: String,
        sub: String,
        artifact_url: Option<String>,
        slide_count: Option<u32>,
    },
    Done,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConductStep {
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u64>,
}

pub fn to_event(ev: &FieldEvent) -> Event {
    Event::default().data(serde_json::to_string(ev).unwrap_or_else(|_| "{}".into()))
}