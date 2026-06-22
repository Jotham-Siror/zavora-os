use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbientAgentRecord {
    pub id: String,
    pub glyph: String,
    pub name: String,
    pub status: String,
    pub task: String,
    pub resolve: Option<serde_json::Value>,
    pub summary: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AmbientEvent {
    Snapshot {
        dnd: bool,
        agents: Vec<AmbientAgentRecord>,
    },
    Update {
        agent: AmbientAgentRecord,
    },
    Dnd {
        enabled: bool,
    },
}

fn default_agents() -> HashMap<String, AmbientAgentRecord> {
    let now = Utc::now();
    [
        (
            "research",
            AmbientAgentRecord {
                id: "research".into(),
                glyph: "🔎".into(),
                name: "Research".into(),
                status: "idle".into(),
                task: "waiting for next cycle".into(),
                resolve: None,
                summary: None,
                updated_at: now,
            },
        ),
        (
            "scout",
            AmbientAgentRecord {
                id: "scout".into(),
                glyph: "🌐".into(),
                name: "Scout".into(),
                status: "idle".into(),
                task: "watching listings".into(),
                resolve: None,
                summary: None,
                updated_at: now,
            },
        ),
        (
            "maker",
            AmbientAgentRecord {
                id: "maker".into(),
                glyph: "🎨".into(),
                name: "Maker".into(),
                status: "idle".into(),
                task: "ready to create".into(),
                resolve: None,
                summary: None,
                updated_at: now,
            },
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

#[derive(Clone)]
pub struct AmbientStore {
    inner: Arc<RwLock<HashMap<String, AmbientAgentRecord>>>,
    dnd: Arc<RwLock<bool>>,
    tx: broadcast::Sender<AmbientEvent>,
}

impl AmbientStore {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(64);
        Self {
            inner: Arc::new(RwLock::new(default_agents())),
            dnd: Arc::new(RwLock::new(false)),
            tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AmbientEvent> {
        self.tx.subscribe()
    }

    pub async fn snapshot(&self) -> (bool, Vec<AmbientAgentRecord>) {
        let dnd = *self.dnd.read().await;
        let agents = self
            .inner
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        (dnd, agents)
    }

    pub async fn set_dnd(&self, enabled: bool) {
        *self.dnd.write().await = enabled;
        let _ = self.tx.send(AmbientEvent::Dnd { enabled });
    }

    pub async fn dnd(&self) -> bool {
        *self.dnd.read().await
    }

    pub async fn set_working(&self, id: &str, task: &str) {
        let mut guard = self.inner.write().await;
        if let Some(rec) = guard.get_mut(id) {
            rec.status = "working".into();
            rec.task = task.into();
            rec.updated_at = Utc::now();
            let _ = self.tx.send(AmbientEvent::Update {
                agent: rec.clone(),
            });
        }
    }

    pub async fn set_done(
        &self,
        id: &str,
        task: &str,
        resolve: serde_json::Value,
        summary: Option<String>,
    ) {
        let mut guard = self.inner.write().await;
        if let Some(rec) = guard.get_mut(id) {
            rec.status = "done".into();
            rec.task = task.into();
            rec.resolve = Some(resolve);
            rec.summary = summary;
            rec.updated_at = Utc::now();
            let _ = self.tx.send(AmbientEvent::Update {
                agent: rec.clone(),
            });
        }
    }

    pub async fn get(&self, id: &str) -> Option<AmbientAgentRecord> {
        self.inner.read().await.get(id).cloned()
    }

    pub async fn list(&self) -> Vec<AmbientAgentRecord> {
        let mut agents: Vec<_> = self.inner.read().await.values().cloned().collect();
        agents.sort_by(|a, b| a.id.cmp(&b.id));
        agents
    }

    pub fn emit_snapshot(&self) {
        let store = self.clone();
        tokio::spawn(async move {
            let (dnd, agents) = store.snapshot().await;
            let _ = store.tx.send(AmbientEvent::Snapshot { dnd, agents });
        });
    }
}

impl Default for AmbientStore {
    fn default() -> Self {
        Self::new()
    }
}