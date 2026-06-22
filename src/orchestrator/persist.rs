use crate::state::{AgentRecord, SessionStore};

pub fn agent_from_card(card: &serde_json::Value) -> AgentRecord {
    let title = card
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Agent")
        .to_string();
    let glyph = card
        .get("glyph")
        .and_then(|v| v.as_str())
        .unwrap_or("✦")
        .to_string();
    let agent = card
        .get("agent")
        .and_then(|v| v.as_str())
        .unwrap_or(&title)
        .to_string();
    AgentRecord {
        id: agent.clone(),
        title,
        glyph,
        agent,
        rail: "active".into(),
    }
}

pub async fn card_spawn(store: &SessionStore, session_id: &str, index: usize, card: serde_json::Value) {
    store
        .upsert_card(session_id, index, card.clone(), "spawn", None, false)
        .await;
    store
        .agent_active(session_id, agent_from_card(&card))
        .await;
}

pub async fn card_resolve(
    store: &SessionStore,
    session_id: &str,
    index: usize,
    card: serde_json::Value,
    resolve: serde_json::Value,
    pinned: bool,
) {
    store
        .upsert_card(
            session_id,
            index,
            card,
            if pinned { "pinned" } else { "resolved" },
            Some(resolve),
            pinned,
        )
        .await;
}