use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use adk_runner::Runner;
use adk_session::InMemorySessionService;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::agents::deck::McpPool;
use crate::ambient::AmbientStore;
use crate::agents::live::LiveMcpPool;
use crate::agents::lisbon::LisbonMcpPool;
use crate::agents::morning::MorningMcpPool;
use crate::agents::people::PeopleMcpPool;
use crate::agents::week::WeekMcpPool;
use crate::scenarios::ScenarioLiveFlags;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionArtifacts {
    pub xlsx: Option<String>,
    pub docx: Option<String>,
    pub pptx: Option<String>,
    pub combined_pptx: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CardRecord {
    pub index: usize,
    pub card: serde_json::Value,
    pub status: String,
    pub resolve: Option<serde_json::Value>,
    pub pinned: bool,
    pub removed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: String,
    pub title: String,
    pub glyph: String,
    pub agent: String,
    pub rail: String,
}

#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub session_id: String,
    pub user_id: String,
    pub scenario: Option<String>,
    pub origin_text: Option<String>,
    pub artifacts: SessionArtifacts,
    pub cards: Vec<CardRecord>,
    pub agents_active: Vec<AgentRecord>,
    pub agents_resting: Vec<AgentRecord>,
}

impl SessionRecord {
    fn new(session_id: String, user_id: String) -> Self {
        Self {
            session_id,
            user_id,
            scenario: None,
            origin_text: None,
            artifacts: SessionArtifacts::default(),
            cards: Vec::new(),
            agents_active: Vec::new(),
            agents_resting: Vec::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct SessionStore {
    inner: Arc<RwLock<HashMap<String, SessionRecord>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create(&self) -> SessionRecord {
        let record = SessionRecord::new(Uuid::new_v4().to_string(), Uuid::new_v4().to_string());
        self.inner
            .write()
            .await
            .insert(record.session_id.clone(), record.clone());
        record
    }

    pub async fn get(&self, session_id: &str) -> Option<SessionRecord> {
        self.inner.read().await.get(session_id).cloned()
    }

    pub async fn set_scenario(&self, session_id: &str, scenario: &str, origin_text: Option<&str>) {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.get_mut(session_id) {
            record.scenario = Some(scenario.into());
            if let Some(text) = origin_text {
                record.origin_text = Some(text.into());
            }
        }
    }

    pub async fn update_artifacts(&self, session_id: &str, artifacts: SessionArtifacts) {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.get_mut(session_id) {
            record.artifacts = artifacts;
        }
    }

    pub async fn upsert_card(
        &self,
        session_id: &str,
        index: usize,
        card: serde_json::Value,
        status: &str,
        resolve: Option<serde_json::Value>,
        pinned: bool,
    ) {
        let mut guard = self.inner.write().await;
        let Some(record) = guard.get_mut(session_id) else {
            return;
        };
        if let Some(existing) = record.cards.iter_mut().find(|c| c.index == index) {
            existing.card = card;
            existing.status = status.into();
            if resolve.is_some() {
                existing.resolve = resolve;
            }
            existing.pinned = pinned;
            return;
        }
        record.cards.push(CardRecord {
            index,
            card,
            status: status.into(),
            resolve,
            pinned,
            removed: false,
        });
        record.cards.sort_by_key(|c| c.index);
    }

    pub async fn remove_card(&self, session_id: &str, title: &str) {
        let mut guard = self.inner.write().await;
        let Some(record) = guard.get_mut(session_id) else {
            return;
        };
        for card in &mut record.cards {
            if card.card.get("title").and_then(|t| t.as_str()) == Some(title) {
                card.removed = true;
            }
        }
    }

    pub async fn list_cards(&self, session_id: &str) -> Option<Vec<CardRecord>> {
        self.get(session_id)
            .await
            .map(|r| r.cards.into_iter().filter(|c| !c.removed).collect())
    }

    pub async fn agent_active(&self, session_id: &str, agent: AgentRecord) {
        let mut guard = self.inner.write().await;
        let Some(record) = guard.get_mut(session_id) else {
            return;
        };
        record.agents_resting.retain(|a| a.id != agent.id);
        if !record.agents_active.iter().any(|a| a.id == agent.id) {
            record.agents_active.push(agent);
        }
    }

    pub async fn agent_snooze(&self, session_id: &str, agent: AgentRecord) {
        let mut guard = self.inner.write().await;
        let Some(record) = guard.get_mut(session_id) else {
            return;
        };
        record.agents_active.retain(|a| a.id != agent.id);
        record.agents_resting.retain(|a| a.id != agent.id);
        record.agents_resting.push(AgentRecord {
            rail: "resting".into(),
            ..agent
        });
    }

    pub async fn agent_wake(&self, session_id: &str, agent_id: &str) -> Option<AgentRecord> {
        let mut guard = self.inner.write().await;
        let record = guard.get_mut(session_id)?;
        let idx = record.agents_resting.iter().position(|a| a.id == agent_id)?;
        let mut agent = record.agents_resting.remove(idx);
        agent.rail = "active".into();
        record.agents_active.retain(|a| a.id != agent_id);
        record.agents_active.push(agent.clone());
        Some(agent)
    }

    pub async fn list_agents(&self, session_id: &str) -> Option<(Vec<AgentRecord>, Vec<AgentRecord>)> {
        let record = self.get(session_id).await?;
        Some((record.agents_active, record.agents_resting))
    }
}

#[derive(Clone)]
pub struct AppState {
    pub sessions: SessionStore,
    pub deck_runner: Option<Arc<Runner>>,
    pub combine_runner: Option<Arc<Runner>>,
    pub morning_runner: Option<Arc<Runner>>,
    pub live_runner: Option<Arc<Runner>>,
    pub people_runner: Option<Arc<Runner>>,
    pub week_runner: Option<Arc<Runner>>,
    pub lisbon_runner: Option<Arc<Runner>>,
    pub router_runner: Option<Arc<Runner>>,
    pub suzy_runner: Option<Arc<Runner>>,
    pub session_service: Arc<InMemorySessionService>,
    pub artifact_dir: PathBuf,
    pub scenario_flags: ScenarioLiveFlags,
    pub coordinator_enabled: bool,
    pub deck_mcp: Option<Arc<McpPool>>,
    pub morning_mcp: Option<Arc<MorningMcpPool>>,
    pub live_mcp: Option<Arc<LiveMcpPool>>,
    pub people_mcp: Option<Arc<PeopleMcpPool>>,
    pub week_mcp: Option<Arc<WeekMcpPool>>,
    pub lisbon_mcp: Option<Arc<LisbonMcpPool>>,
    pub ambient: AmbientStore,
    pub ambient_enabled: bool,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        artifact_dir: PathBuf,
        scenario_flags: ScenarioLiveFlags,
        coordinator_enabled: bool,
        deck_runner: Option<Arc<Runner>>,
        combine_runner: Option<Arc<Runner>>,
        morning_runner: Option<Arc<Runner>>,
        live_runner: Option<Arc<Runner>>,
        people_runner: Option<Arc<Runner>>,
        week_runner: Option<Arc<Runner>>,
        lisbon_runner: Option<Arc<Runner>>,
        router_runner: Option<Arc<Runner>>,
        suzy_runner: Option<Arc<Runner>>,
        session_service: Arc<InMemorySessionService>,
        deck_mcp: Option<Arc<McpPool>>,
        morning_mcp: Option<Arc<MorningMcpPool>>,
        live_mcp: Option<Arc<LiveMcpPool>>,
        people_mcp: Option<Arc<PeopleMcpPool>>,
        week_mcp: Option<Arc<WeekMcpPool>>,
        lisbon_mcp: Option<Arc<LisbonMcpPool>>,
        ambient: AmbientStore,
        ambient_enabled: bool,
    ) -> Self {
        Self {
            sessions: SessionStore::new(),
            deck_runner,
            combine_runner,
            morning_runner,
            live_runner,
            people_runner,
            week_runner,
            lisbon_runner,
            router_runner,
            suzy_runner,
            session_service,
            artifact_dir,
            scenario_flags,
            coordinator_enabled,
            deck_mcp,
            morning_mcp,
            live_mcp,
            people_mcp,
            week_mcp,
            lisbon_mcp,
            ambient,
            ambient_enabled,
        }
    }
}