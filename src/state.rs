use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

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
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: SessionStore::new(),
        }
    }
}