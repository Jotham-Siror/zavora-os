//! The permission gate — a `Toolset` wrapper that enforces Observe / Suggest / Automate at the
//! tool boundary (S2-T5, ADR-003).
//!
//! ```text
//! LlmAgent ─▶ PermissionGate(agent, mode, effects) ─▶ FilteredToolset ─▶ MCP
//!                ├─ read ............ pass through, ledger
//!                ├─ write_local ..... pass if mode ≥ suggest, ledger + audit
//!                ├─ other effects ... automate & recipe → run, audit
//!                │                    suggest → enqueue pending_action, audit "queued"
//!                └─ observe & effect ≠ read → "not permitted in observe mode"
//! ```
//!
//! The gate needs the permission store, pending queue, audit log and ledger at execution time,
//! long after the agents were built at boot, so those live in a process-wide
//! [`PermissionServices`] handle (`init` at boot; an in-memory default otherwise, e.g. tests).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::domain::Domain;
use crate::intelligence::ledger::{ActivityEvent, LedgerService};
use crate::permissions::audit::{AuditEntry, AuditLog};
use crate::permissions::pending::PendingActions;
use crate::permissions::store::PermissionStore;
use crate::permissions::{decide, Decision, Effect, Mode};
use crate::tools::allowlist;

type RegistryMap = HashMap<(String, String), Arc<dyn adk_core::Tool>>;

/// Un-gated tools by `(agent_id, tool_name)` so approved pending actions can be executed later.
#[derive(Clone, Default)]
pub struct ToolRegistry {
    inner: Arc<RwLock<RegistryMap>>,
}

impl ToolRegistry {
    pub async fn register(&self, agent_id: &str, tool: Arc<dyn adk_core::Tool>) {
        self.inner
            .write()
            .await
            .insert((agent_id.to_string(), tool.name().to_string()), tool);
    }
    pub async fn get(&self, agent_id: &str, tool: &str) -> Option<Arc<dyn adk_core::Tool>> {
        self.inner.read().await.get(&(agent_id.to_string(), tool.to_string())).cloned()
    }
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

#[derive(Clone)]
pub struct PermissionServices {
    pub permissions: PermissionStore,
    pub pending: PendingActions,
    pub audit: AuditLog,
    pub ledger: LedgerService,
    pub registry: ToolRegistry,
}

impl PermissionServices {
    pub fn in_memory() -> Self {
        Self {
            permissions: PermissionStore::in_memory(),
            pending: PendingActions::in_memory(),
            audit: AuditLog::in_memory(),
            ledger: LedgerService::in_memory(),
            registry: ToolRegistry::default(),
        }
    }

    pub fn with_postgres(pool: sqlx::PgPool, ledger_key: Vec<u8>) -> Self {
        Self {
            permissions: PermissionStore::new(Some(pool.clone())),
            pending: PendingActions::new(Some(pool.clone())),
            audit: AuditLog::new(Some(pool.clone())),
            ledger: LedgerService::new(Some(pool), ledger_key),
            registry: ToolRegistry::default(),
        }
    }
}

static SERVICES: OnceLock<PermissionServices> = OnceLock::new();

/// Install the process-wide services. Call once at boot before agents are built.
pub fn init(services: PermissionServices) -> Result<(), PermissionServices> {
    SERVICES.set(services)
}

/// The process-wide services (an in-memory default when `init` was never called).
pub fn services() -> &'static PermissionServices {
    SERVICES.get_or_init(PermissionServices::in_memory)
}

/// Toolset wrapper: every tool of `inner` becomes a [`GatedTool`] for `agent_id`.
pub struct PermissionGate {
    agent_id: String,
    domain: Domain,
    inner: Arc<dyn adk_core::Toolset>,
}

impl PermissionGate {
    pub fn wrap(agent_id: &str, inner: Arc<dyn adk_core::Toolset>) -> Arc<dyn adk_core::Toolset> {
        Arc::new(Self {
            agent_id: agent_id.to_string(),
            domain: allowlist::catalog().world_for(agent_id),
            inner,
        })
    }
}

#[async_trait]
impl adk_core::Toolset for PermissionGate {
    fn name(&self) -> &str {
        self.inner.name()
    }

    async fn tools(
        &self,
        ctx: Arc<dyn adk_core::ReadonlyContext>,
    ) -> adk_core::Result<Vec<Arc<dyn adk_core::Tool>>> {
        let tools = self.inner.tools(ctx).await?;
        let registry = &services().registry;
        let mut out: Vec<Arc<dyn adk_core::Tool>> = Vec::with_capacity(tools.len());
        for t in tools {
            registry.register(&self.agent_id, t.clone()).await;
            out.push(Arc::new(GatedTool {
                agent_id: self.agent_id.clone(),
                domain: self.domain,
                effect: allowlist::effect_for(&self.agent_id, t.name()),
                inner: t,
            }));
        }
        Ok(out)
    }
}

/// One gated tool. Read effects pass; everything else consults the mode matrix.
pub struct GatedTool {
    agent_id: String,
    domain: Domain,
    effect: Effect,
    inner: Arc<dyn adk_core::Tool>,
}

/// Keys whose value identifies a subject worth correlating (hashed, never stored raw).
const SUBJECT_KEYS: &[&str] = &["thread_id", "message_id", "email_id", "event_id", "id", "task_id", "channel"];

fn subject_of(args: &serde_json::Value) -> Option<String> {
    let obj = args.as_object()?;
    for k in SUBJECT_KEYS {
        if let Some(v) = obj.get(*k) {
            if let Some(s) = v.as_str() {
                return Some(s.to_string());
            }
            if v.is_number() {
                return Some(v.to_string());
            }
        }
    }
    None
}

impl GatedTool {
    pub fn effect(&self) -> Effect {
        self.effect
    }

    fn ledger_event(&self, user: &str, decision: &str, mode: Mode, args: &serde_json::Value, duration_ms: Option<i32>) -> ActivityEvent {
        let svc = services();
        let mut ev = ActivityEvent::new(user, self.domain, &self.agent_id, "tool_call")
            .effect(self.effect)
            .meta(serde_json::json!({
                "tool_class": self.effect.as_str(),
                "decision": decision,
                "mode": mode.as_str(),
            }));
        if let Some(ms) = duration_ms {
            ev = ev.duration_ms(ms);
        }
        if let Some(subject) = subject_of(args) {
            ev = ev.subject(svc.ledger.hash_key(), &subject);
        }
        ev
    }
}

#[async_trait]
impl adk_core::Tool for GatedTool {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn description(&self) -> &str {
        self.inner.description()
    }
    fn parameters_schema(&self) -> Option<serde_json::Value> {
        self.inner.parameters_schema()
    }
    fn response_schema(&self) -> Option<serde_json::Value> {
        self.inner.response_schema()
    }
    fn is_long_running(&self) -> bool {
        self.inner.is_long_running()
    }
    fn is_read_only(&self) -> bool {
        self.effect.is_read()
    }
    fn is_concurrency_safe(&self) -> bool {
        self.effect.is_read() && self.inner.is_concurrency_safe()
    }

    async fn execute(
        &self,
        ctx: Arc<dyn adk_core::ToolContext>,
        args: serde_json::Value,
    ) -> adk_core::Result<serde_json::Value> {
        let svc = services();
        let user = ctx.user_id().to_string();
        let session = ctx.session_id().to_string();
        let session_opt = (!session.is_empty()).then_some(session.as_str());
        let tool = self.inner.name().to_string();

        // Global / per-world pause: reads still work, writes wait.
        if !self.effect.is_read() && svc.permissions.is_paused(self.domain).await {
            let mode = svc.permissions.mode_for(&user, &self.agent_id, Some(&tool)).await;
            svc.audit
                .record(AuditEntry::new(&user, session_opt, &self.agent_id, self.domain, &tool, self.effect, "paused", mode, format!("{} · {tool} ({}) while paused", self.agent_id, self.effect)))
                .await;
            svc.ledger.record(self.ledger_event(&user, "paused", mode, &args, None));
            return Ok(serde_json::json!({
                "status": "paused",
                "message": "The user has paused this world's agents. Do not retry; describe what you would do instead."
            }));
        }

        let mode = svc.permissions.mode_for(&user, &self.agent_id, Some(&tool)).await;
        match decide(mode, self.effect, false) {
            Decision::Allow => {
                let started = Instant::now();
                let result = self.inner.execute(ctx, args.clone()).await;
                let ms = started.elapsed().as_millis().min(i32::MAX as u128) as i32;
                let decision = if result.is_ok() { "allowed" } else { "failed" };
                if !self.effect.is_read() {
                    svc.audit
                        .record(AuditEntry::new(&user, session_opt, &self.agent_id, self.domain, &tool, self.effect, decision, mode, crate::permissions::pending::summarize(&self.agent_id, &tool, self.effect, &args)))
                        .await;
                }
                svc.ledger.record(self.ledger_event(&user, decision, mode, &args, Some(ms)));
                result
            }
            Decision::Pending => {
                let action = svc
                    .pending
                    .create(&user, session_opt, &self.agent_id, self.domain, &tool, self.effect, args.clone(), None)
                    .await;
                svc.audit
                    .record(
                        AuditEntry::new(&user, session_opt, &self.agent_id, self.domain, &tool, self.effect, "queued", mode, action.summary.clone())
                            .approval(action.id),
                    )
                    .await;
                svc.ledger.record(self.ledger_event(&user, "queued", mode, &args, None));
                Ok(serde_json::json!({
                    "status": "queued_for_approval",
                    "action_id": action.id,
                    "effect": self.effect.as_str(),
                    "expires_at": action.expires_at,
                    "message": format!(
                        "{tool} is a {} effect and this agent runs in {} mode, so it was queued for the user's approval. \
                         Do not call it again. Tell the user what is waiting for their approval.",
                        self.effect, mode
                    )
                }))
            }
            Decision::Deny(reason) => {
                svc.audit
                    .record(AuditEntry::new(&user, session_opt, &self.agent_id, self.domain, &tool, self.effect, "denied", mode, format!("{} · {tool} ({}) denied: {reason}", self.agent_id, self.effect)))
                    .await;
                svc.ledger.record(self.ledger_event(&user, "denied", mode, &args, None));
                Ok(serde_json::json!({
                    "status": "denied",
                    "effect": self.effect.as_str(),
                    "mode": mode.as_str(),
                    "message": format!("{tool} ({}) is {reason}.", self.effect)
                }))
            }
        }
    }
}

/// Execute an approved pending action with the un-gated tool (the user *is* the approval).
pub async fn execute_approved(
    action: &crate::permissions::pending::PendingAction,
) -> Result<serde_json::Value, String> {
    let svc = services();
    let Some(tool) = svc.registry.get(&action.agent_id, &action.tool).await else {
        return Err(format!(
            "tool {} for agent {} is not available — its MCP server is not connected",
            action.tool, action.agent_id
        ));
    };
    let mut ctx = adk_tool::SimpleToolContext::new("approval");
    if let Some(sid) = &action.session_id {
        ctx = ctx.with_session_id(sid.clone());
    }
    tool.execute(Arc::new(ctx) as Arc<dyn adk_core::ToolContext>, action.args.clone())
        .await
        .map_err(|e| e.to_string())
}
