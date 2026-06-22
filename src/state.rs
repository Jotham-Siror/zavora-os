use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use adk_runner::Runner;
use adk_session::InMemorySessionService;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::agents::deck::McpPool;

#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub session_id: String,
    pub user_id: String,
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
}

#[derive(Clone)]
pub struct AppState {
    pub sessions: SessionStore,
    pub runner: Option<Arc<Runner>>,
    pub session_service: Arc<InMemorySessionService>,
    pub artifact_dir: PathBuf,
    pub deck_enabled: bool,
    pub mcp_pool: Option<Arc<McpPool>>,
}

impl AppState {
    pub fn new(
        artifact_dir: PathBuf,
        deck_enabled: bool,
        runner: Option<Arc<Runner>>,
        session_service: Arc<InMemorySessionService>,
        mcp_pool: Option<Arc<McpPool>>,
    ) -> Self {
        Self {
            sessions: SessionStore::new(),
            runner,
            session_service,
            artifact_dir,
            deck_enabled,
            mcp_pool,
        }
    }
}