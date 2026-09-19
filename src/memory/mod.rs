//! Personal memory — known · assumed · recommended (concept §11, S3).
//!
//! - [`service`]: scoped read, remember (known), propose (assumed), recommend, confirm,
//!   correct, forget, export, purge; provenance on every change; encryption at rest for
//!   sensitive values ([`crypto`]).
//! - [`tools`]: `read_memory` / `propose_memory` for agents, scoped by world.
//! - [`chat`]: "remember …", "forget …", "why do you think …" handled before intake.
//! - [`consent`]: persisted consents per category and world (S11-T4, pulled forward).
//!
//! Like the permission services, the memory service is installed process-wide at boot so
//! agents built before `AppState` exists can reach it.

pub mod chat;
pub mod consent;
pub mod crypto;
pub mod service;
pub mod tools;

use std::sync::OnceLock;

pub use service::{Kind, MemoryItem, MemoryService, NewItem, Provenance, Scope, Sensitivity};

static SERVICE: OnceLock<MemoryService> = OnceLock::new();

/// Install the process-wide memory service (boot). Errors if already installed.
pub fn init(service: MemoryService) -> Result<(), MemoryService> {
    SERVICE.set(service)
}

/// The process-wide memory service (an in-memory default when `init` was never called).
pub fn service_handle() -> &'static MemoryService {
    SERVICE.get_or_init(MemoryService::in_memory)
}
