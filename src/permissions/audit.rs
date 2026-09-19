//! Audit log — immutable record of every executed, queued, approved, rejected or denied effect
//! (S2-T6). Summaries are content-free (agent, tool, effect, argument keys).

use std::collections::VecDeque;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::domain::Domain;
use crate::permissions::{Effect, Mode};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub user_id: String,
    pub session_id: Option<String>,
    pub agent_id: String,
    pub domain: Domain,
    pub tool: String,
    pub effect: Effect,
    /// allowed | queued | approved | rejected | denied | failed | paused
    pub decision: String,
    pub approval_id: Option<Uuid>,
    pub recipe_id: Option<Uuid>,
    pub mode: Mode,
    pub summary: String,
    pub undo_token: Option<String>,
    pub undone_at: Option<DateTime<Utc>>,
    pub trace_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl AuditEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_id: &str,
        session_id: Option<&str>,
        agent_id: &str,
        domain: Domain,
        tool: &str,
        effect: Effect,
        decision: &str,
        mode: Mode,
        summary: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id: user_id.into(),
            session_id: session_id.map(str::to_string),
            agent_id: agent_id.into(),
            domain,
            tool: tool.into(),
            effect,
            decision: decision.into(),
            approval_id: None,
            recipe_id: None,
            mode,
            summary,
            undo_token: None,
            undone_at: None,
            trace_id: None,
            created_at: Utc::now(),
        }
    }
    pub fn approval(mut self, id: Uuid) -> Self {
        self.approval_id = Some(id);
        self
    }
    pub fn trace(mut self, trace_id: Option<&str>) -> Self {
        self.trace_id = trace_id.map(str::to_string);
        self
    }
}

const RING: usize = 2_000;

#[derive(Clone)]
pub struct AuditLog {
    inner: Arc<RwLock<VecDeque<AuditEntry>>>,
    pg: Option<PgPool>,
}

impl AuditLog {
    pub fn new(pg: Option<PgPool>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(VecDeque::with_capacity(RING))),
            pg,
        }
    }

    pub fn in_memory() -> Self {
        Self::new(None)
    }

    pub async fn record(&self, entry: AuditEntry) -> AuditEntry {
        {
            let mut guard = self.inner.write().await;
            if guard.len() >= RING {
                guard.pop_front();
            }
            guard.push_back(entry.clone());
        }
        if let Some(pool) = &self.pg
            && let Err(e) = sqlx::query(
                "INSERT INTO audit_log (id, user_id, session_id, agent_id, domain, tool, effect, decision, approval_id, recipe_id, mode, summary, undo_token, trace_id, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
            )
            .bind(entry.id)
            .bind(&entry.user_id)
            .bind(&entry.session_id)
            .bind(&entry.agent_id)
            .bind(entry.domain.as_str())
            .bind(&entry.tool)
            .bind(entry.effect.as_str())
            .bind(&entry.decision)
            .bind(entry.approval_id)
            .bind(entry.recipe_id)
            .bind(entry.mode.as_str())
            .bind(&entry.summary)
            .bind(&entry.undo_token)
            .bind(&entry.trace_id)
            .bind(entry.created_at)
            .execute(pool)
            .await
        {
            tracing::warn!("audit_log insert failed: {e:#}");
        }
        entry
    }

    /// Newest first.
    pub async fn list(&self, user_id: &str, limit: usize) -> Vec<AuditEntry> {
        self.inner
            .read()
            .await
            .iter()
            .rev()
            .filter(|e| e.user_id == user_id)
            .take(if limit == 0 { usize::MAX } else { limit })
            .cloned()
            .collect()
    }

    pub async fn count(&self, user_id: &str, decision: Option<&str>) -> usize {
        self.inner
            .read()
            .await
            .iter()
            .filter(|e| e.user_id == user_id && decision.map(|d| e.decision == d).unwrap_or(true))
            .count()
    }
}
