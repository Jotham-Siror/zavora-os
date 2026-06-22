use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use adk_runner::Runner;
use adk_session::InMemorySessionService;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::agents::deck::McpPool;
use crate::agents::morning::MorningMcpPool;

#[derive(Clone, Debug, Default)]
pub struct SessionArtifacts {
    pub xlsx: Option<String>,
    pub docx: Option<String>,
    pub pptx: Option<String>,
    pub combined_pptx: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub session_id: String,
    pub user_id: String,
    pub scenario: Option<String>,
    pub artifacts: SessionArtifacts,
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
        let record = SessionRecord {
            session_id: Uuid::new_v4().to_string(),
            user_id: Uuid::new_v4().to_string(),
            scenario: None,
            artifacts: SessionArtifacts::default(),
        };
        self.inner
            .write()
            .await
            .insert(record.session_id.clone(), record.clone());
        record
    }

    pub async fn get(&self, session_id: &str) -> Option<SessionRecord> {
        self.inner.read().await.get(session_id).cloned()
    }

    pub async fn set_scenario(&self, session_id: &str, scenario: &str) {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.get_mut(session_id) {
            record.scenario = Some(scenario.into());
        }
    }

    pub async fn update_artifacts(&self, session_id: &str, artifacts: SessionArtifacts) {
        let mut guard = self.inner.write().await;
        if let Some(record) = guard.get_mut(session_id) {
            record.artifacts = artifacts;
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub sessions: SessionStore,
    pub deck_runner: Option<Arc<Runner>>,
    pub combine_runner: Option<Arc<Runner>>,
    pub morning_runner: Option<Arc<Runner>>,
    pub session_service: Arc<InMemorySessionService>,
    pub artifact_dir: PathBuf,
    pub deck_enabled: bool,
    pub morning_enabled: bool,
    pub deck_mcp: Option<Arc<McpPool>>,
    pub morning_mcp: Option<Arc<MorningMcpPool>>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        artifact_dir: PathBuf,
        deck_enabled: bool,
        morning_enabled: bool,
        deck_runner: Option<Arc<Runner>>,
        combine_runner: Option<Arc<Runner>>,
        morning_runner: Option<Arc<Runner>>,
        session_service: Arc<InMemorySessionService>,
        deck_mcp: Option<Arc<McpPool>>,
        morning_mcp: Option<Arc<MorningMcpPool>>,
    ) -> Self {
        Self {
            sessions: SessionStore::new(),
            deck_runner,
            combine_runner,
            morning_runner,
            session_service,
            artifact_dir,
            deck_enabled,
            morning_enabled,
            deck_mcp,
            morning_mcp,
        }
    }
}