//! Pending actions — proposed effects waiting for the user (S2-T6/T7).

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::domain::Domain;
use crate::permissions::Effect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PendingStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
    Failed,
}

impl PendingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PendingStatus::Pending => "pending",
            PendingStatus::Approved => "approved",
            PendingStatus::Rejected => "rejected",
            PendingStatus::Expired => "expired",
            PendingStatus::Failed => "failed",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "expired" => Some(Self::Expired),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingAction {
    pub id: Uuid,
    pub user_id: String,
    pub session_id: Option<String>,
    pub agent_id: String,
    pub domain: Domain,
    pub tool: String,
    pub effect: Effect,
    /// The exact tool arguments to run on approval. Content by nature; never copied into the
    /// ledger or audit summary. Encrypted at rest from S3 for sensitive agents.
    pub args: serde_json::Value,
    /// Content-free description: agent, tool, effect, argument *keys*.
    pub summary: String,
    pub status: PendingStatus,
    pub trace_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub result: Option<serde_json::Value>,
}

/// Default lifetime of a pending action (§10.4).
pub const PENDING_TTL_HOURS: i64 = 48;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PendingEvent {
    Created(PendingAction),
    Resolved(PendingAction),
}

/// Argument keys only — a content-free summary for audit and SSE.
pub fn summarize(agent_id: &str, tool: &str, effect: Effect, args: &serde_json::Value) -> String {
    let keys = args
        .as_object()
        .map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    if keys.is_empty() {
        format!("{agent_id} · {tool} ({effect})")
    } else {
        format!("{agent_id} · {tool} ({effect}) with {keys}")
    }
}

#[derive(Clone)]
pub struct PendingActions {
    inner: Arc<RwLock<Vec<PendingAction>>>,
    pg: Option<PgPool>,
    tx: broadcast::Sender<PendingEvent>,
}

impl PendingActions {
    pub fn new(pg: Option<PgPool>) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(RwLock::new(Vec::new())),
            pg,
            tx,
        }
    }

    pub fn in_memory() -> Self {
        Self::new(None)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PendingEvent> {
        self.tx.subscribe()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        user_id: &str,
        session_id: Option<&str>,
        agent_id: &str,
        domain: Domain,
        tool: &str,
        effect: Effect,
        args: serde_json::Value,
        trace_id: Option<&str>,
    ) -> PendingAction {
        let now = Utc::now();
        let action = PendingAction {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            session_id: session_id.map(str::to_string),
            agent_id: agent_id.into(),
            domain,
            tool: tool.into(),
            effect,
            summary: summarize(agent_id, tool, effect, &args),
            args,
            status: PendingStatus::Pending,
            trace_id: trace_id.map(str::to_string),
            created_at: now,
            expires_at: now + Duration::hours(PENDING_TTL_HOURS),
            resolved_at: None,
            result: None,
        };
        self.inner.write().await.push(action.clone());
        if let Some(pool) = &self.pg
            && let Err(e) = insert_pg(pool, &action).await
        {
            tracing::warn!("pending_actions insert failed: {e:#}");
        }
        let _ = self.tx.send(PendingEvent::Created(action.clone()));
        action
    }

    pub async fn get(&self, id: Uuid) -> Option<PendingAction> {
        self.inner.read().await.iter().find(|a| a.id == id).cloned()
    }

    /// List actions for a user, optionally filtered by status and session. Expires stale ones.
    pub async fn list(&self, user_id: &str, status: Option<PendingStatus>, session_id: Option<&str>) -> Vec<PendingAction> {
        self.expire_stale().await;
        self.inner
            .read()
            .await
            .iter()
            .filter(|a| a.user_id == user_id)
            .filter(|a| status.map(|s| a.status == s).unwrap_or(true))
            .filter(|a| session_id.map(|s| a.session_id.as_deref() == Some(s)).unwrap_or(true))
            .cloned()
            .collect()
    }

    pub async fn resolve(&self, id: Uuid, status: PendingStatus, result: Option<serde_json::Value>) -> Option<PendingAction> {
        let updated = {
            let mut guard = self.inner.write().await;
            let action = guard.iter_mut().find(|a| a.id == id)?;
            if action.status != PendingStatus::Pending {
                return Some(action.clone());
            }
            action.status = status;
            action.resolved_at = Some(Utc::now());
            action.result = result;
            action.clone()
        };
        if let Some(pool) = &self.pg
            && let Err(e) = sqlx::query("UPDATE pending_actions SET status = $2, resolved_at = $3, result = $4 WHERE id = $1")
                .bind(updated.id)
                .bind(updated.status.as_str())
                .bind(updated.resolved_at)
                .bind(&updated.result)
                .execute(pool)
                .await
        {
            tracing::warn!("pending_actions update failed: {e:#}");
        }
        let _ = self.tx.send(PendingEvent::Resolved(updated.clone()));
        Some(updated)
    }

    /// Replace the arguments of a pending action (the "Edit" path) — stays pending.
    pub async fn edit(&self, id: Uuid, args: serde_json::Value) -> Option<PendingAction> {
        let mut guard = self.inner.write().await;
        let action = guard.iter_mut().find(|a| a.id == id && a.status == PendingStatus::Pending)?;
        action.summary = summarize(&action.agent_id, &action.tool, action.effect, &args);
        action.args = args;
        Some(action.clone())
    }

    pub async fn expire_stale(&self) {
        let now = Utc::now();
        let mut expired = Vec::new();
        {
            let mut guard = self.inner.write().await;
            for a in guard.iter_mut() {
                if a.status == PendingStatus::Pending && a.expires_at <= now {
                    a.status = PendingStatus::Expired;
                    a.resolved_at = Some(now);
                    expired.push(a.clone());
                }
            }
        }
        for a in expired {
            let _ = self.tx.send(PendingEvent::Resolved(a));
        }
    }
}

async fn insert_pg(pool: &PgPool, a: &PendingAction) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO pending_actions (id, user_id, session_id, agent_id, domain, tool, effect, args, summary, status, trace_id, created_at, expires_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
    )
    .bind(a.id)
    .bind(&a.user_id)
    .bind(&a.session_id)
    .bind(&a.agent_id)
    .bind(a.domain.as_str())
    .bind(&a.tool)
    .bind(a.effect.as_str())
    .bind(&a.args)
    .bind(&a.summary)
    .bind(a.status.as_str())
    .bind(&a.trace_id)
    .bind(a.created_at)
    .bind(a.expires_at)
    .execute(pool)
    .await?;
    Ok(())
}
