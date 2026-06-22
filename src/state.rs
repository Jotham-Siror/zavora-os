use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use adk_runner::Runner;
use adk_session::SessionService;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::agents::deck::McpPool;
use adk_awp::InMemoryEventSubscriptionService;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

pub type SharedSessionService = Arc<dyn SessionService + Send + Sync>;

#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<RwLock<HashMap<String, SessionRecord>>>,
    pg: Option<PgPool>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            pg: None,
        }
    }

    pub fn with_postgres(pool: PgPool) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            pg: Some(pool),
        }
    }

    pub fn postgres_enabled(&self) -> bool {
        self.pg.is_some()
    }

    pub async fn create(&self) -> SessionRecord {
        self.create_for_user(Uuid::new_v4().to_string()).await
    }

    pub async fn create_for_user(&self, user_id: String) -> SessionRecord {
        let record = SessionRecord::new(Uuid::new_v4().to_string(), user_id);
        self.save(&record).await;
        record
    }

    pub async fn get(&self, session_id: &str) -> Option<SessionRecord> {
        if let Some(pool) = &self.pg {
            if let Ok(Some(record)) = load_ui_session(pool, session_id).await {
                self.inner
                    .write()
                    .await
                    .insert(session_id.to_string(), record.clone());
                return Some(record);
            }
        }
        self.inner.read().await.get(session_id).cloned()
    }

    async fn save(&self, record: &SessionRecord) {
        self.inner
            .write()
            .await
            .insert(record.session_id.clone(), record.clone());
        if let Some(pool) = &self.pg {
            if let Err(e) = upsert_ui_session(pool, record).await {
                tracing::warn!("ui_session persist failed: {e:#}");
            }
        }
    }

    async fn mutate<F>(&self, session_id: &str, f: F)
    where
        F: FnOnce(&mut SessionRecord),
    {
        let mut guard = self.inner.write().await;
        let Some(record) = guard.get_mut(session_id) else {
            return;
        };
        f(record);
        let snapshot = record.clone();
        drop(guard);
        if let Some(pool) = &self.pg {
            if let Err(e) = upsert_ui_session(pool, &snapshot).await {
                tracing::warn!("ui_session persist failed: {e:#}");
            }
        }
    }

    pub async fn set_scenario(&self, session_id: &str, scenario: &str, origin_text: Option<&str>) {
        self.mutate(session_id, |record| {
            record.scenario = Some(scenario.into());
            if let Some(text) = origin_text {
                record.origin_text = Some(text.into());
            }
        })
        .await;
    }

    pub async fn update_artifacts(&self, session_id: &str, artifacts: SessionArtifacts) {
        self.mutate(session_id, |record| {
            record.artifacts = artifacts;
        })
        .await;
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
        self.mutate(session_id, |record| {
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
        })
        .await;
    }

    pub async fn remove_card(&self, session_id: &str, title: &str) {
        self.mutate(session_id, |record| {
            for card in &mut record.cards {
                if card.card.get("title").and_then(|t| t.as_str()) == Some(title) {
                    card.removed = true;
                }
            }
        })
        .await;
    }

    pub async fn list_cards(&self, session_id: &str) -> Option<Vec<CardRecord>> {
        self.get(session_id)
            .await
            .map(|r| r.cards.into_iter().filter(|c| !c.removed).collect())
    }

    pub async fn agent_active(&self, session_id: &str, agent: AgentRecord) {
        self.mutate(session_id, |record| {
            record.agents_resting.retain(|a| a.id != agent.id);
            if !record.agents_active.iter().any(|a| a.id == agent.id) {
                record.agents_active.push(agent);
            }
        })
        .await;
    }

    pub async fn agent_snooze(&self, session_id: &str, agent: AgentRecord) {
        self.mutate(session_id, |record| {
            record.agents_active.retain(|a| a.id != agent.id);
            record.agents_resting.retain(|a| a.id != agent.id);
            record.agents_resting.push(AgentRecord {
                rail: "resting".into(),
                ..agent
            });
        })
        .await;
    }

    pub async fn agent_wake(&self, session_id: &str, agent_id: &str) -> Option<AgentRecord> {
        let mut guard = self.inner.write().await;
        let record = guard.get_mut(session_id)?;
        let idx = record.agents_resting.iter().position(|a| a.id == agent_id)?;
        let mut agent = record.agents_resting.remove(idx);
        agent.rail = "active".into();
        record.agents_active.retain(|a| a.id != agent_id);
        record.agents_active.push(agent.clone());
        let snapshot = record.clone();
        drop(guard);
        if let Some(pool) = &self.pg {
            if let Err(e) = upsert_ui_session(pool, &snapshot).await {
                tracing::warn!("ui_session persist failed: {e:#}");
            }
        }
        Some(agent)
    }

    pub async fn list_agents(&self, session_id: &str) -> Option<(Vec<AgentRecord>, Vec<AgentRecord>)> {
        let record = self.get(session_id).await?;
        Some((record.agents_active, record.agents_resting))
    }
}

async fn load_ui_session(pool: &PgPool, session_id: &str) -> anyhow::Result<Option<SessionRecord>> {
    let row = sqlx::query_as::<_, UiSessionRow>(
        "SELECT session_id, user_id, scenario, origin_text, artifacts, cards, agents_active, agents_resting FROM ui_sessions WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| SessionRecord {
        session_id: r.session_id,
        user_id: r.user_id,
        scenario: r.scenario,
        origin_text: r.origin_text,
        artifacts: serde_json::from_value(r.artifacts).unwrap_or_default(),
        cards: serde_json::from_value(r.cards).unwrap_or_default(),
        agents_active: serde_json::from_value(r.agents_active).unwrap_or_default(),
        agents_resting: serde_json::from_value(r.agents_resting).unwrap_or_default(),
    }))
}

async fn upsert_ui_session(pool: &PgPool, record: &SessionRecord) -> anyhow::Result<()> {
    let artifacts = serde_json::to_value(&record.artifacts)?;
    let cards = serde_json::to_value(&record.cards)?;
    let agents_active = serde_json::to_value(&record.agents_active)?;
    let agents_resting = serde_json::to_value(&record.agents_resting)?;

    sqlx::query(
        "INSERT INTO ui_sessions (session_id, user_id, scenario, origin_text, artifacts, cards, agents_active, agents_resting) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (session_id) DO UPDATE SET user_id = $2, scenario = $3, origin_text = $4, artifacts = $5, cards = $6, agents_active = $7, agents_resting = $8, updated_at = NOW()",
    )
    .bind(&record.session_id)
    .bind(&record.user_id)
    .bind(&record.scenario)
    .bind(&record.origin_text)
    .bind(artifacts)
    .bind(cards)
    .bind(agents_active)
    .bind(agents_resting)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct UiSessionRow {
    session_id: String,
    user_id: String,
    scenario: Option<String>,
    origin_text: Option<String>,
    artifacts: serde_json::Value,
    cards: serde_json::Value,
    agents_active: serde_json::Value,
    agents_resting: serde_json::Value,
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
    pub session_service: SharedSessionService,
    pub auth: Option<Arc<crate::auth::AuthState>>,
    pub awp: Arc<crate::awp_gate::AwpGate>,
    pub event_service: Arc<InMemoryEventSubscriptionService>,
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
    pub greeting_runner: Option<Arc<Runner>>,
    pub brand_greeting_body: String,
    pub brand_tone: Option<String>,
    pub voice: crate::voice::VoiceState,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sessions: SessionStore,
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
        session_service: SharedSessionService,
        auth: Option<Arc<crate::auth::AuthState>>,
        awp: Arc<crate::awp_gate::AwpGate>,
        event_service: Arc<InMemoryEventSubscriptionService>,
        deck_mcp: Option<Arc<McpPool>>,
        morning_mcp: Option<Arc<MorningMcpPool>>,
        live_mcp: Option<Arc<LiveMcpPool>>,
        people_mcp: Option<Arc<PeopleMcpPool>>,
        week_mcp: Option<Arc<WeekMcpPool>>,
        lisbon_mcp: Option<Arc<LisbonMcpPool>>,
        ambient: AmbientStore,
        ambient_enabled: bool,
        greeting_runner: Option<Arc<Runner>>,
        brand_greeting_body: String,
        brand_tone: Option<String>,
        voice: crate::voice::VoiceState,
    ) -> Self {
        Self {
            sessions,
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
            auth,
            awp,
            event_service,
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
            greeting_runner,
            brand_greeting_body,
            brand_tone,
            voice,
        }
    }
}