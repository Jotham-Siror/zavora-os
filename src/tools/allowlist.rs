//! MCP Registry-compatible allowlist catalog (BK-011).
//!
//! `mcp_allowlists.toml` is the version-controlled source of truth; optional
//! `MCP_REGISTRY_PATH` syncs entries to the registry MCP at boot.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::Context;
use serde::Deserialize;

static CATALOG: OnceLock<AllowlistCatalog> = OnceLock::new();

#[derive(Debug, Clone, Deserialize)]
struct AllowlistFile {
    allowlist: Vec<AllowlistEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AllowlistEntry {
    pub agent: String,
    pub mcp_server: String,
    #[serde(default)]
    pub reason: String,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AllowlistCatalog {
    by_agent: HashMap<String, HashSet<String>>,
    entries: Vec<AllowlistEntry>,
}

impl AllowlistCatalog {
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read allowlist catalog {}", path.display()))?;
        let file: AllowlistFile = toml::from_str(&raw)
            .with_context(|| format!("parse allowlist catalog {}", path.display()))?;
        Ok(Self::from_entries(file.allowlist))
    }

    pub fn from_entries(entries: Vec<AllowlistEntry>) -> Self {
        let mut by_agent: HashMap<String, HashSet<String>> = HashMap::new();
        for entry in &entries {
            by_agent
                .entry(entry.agent.clone())
                .or_default()
                .extend(entry.tools.iter().cloned());
        }
        Self { by_agent, entries }
    }

    pub fn tools_for_agent(&self, agent: &str) -> Vec<String> {
        self.by_agent
            .get(agent)
            .map(|set| {
                let mut tools: Vec<String> = set.iter().cloned().collect();
                tools.sort();
                tools
            })
            .unwrap_or_default()
    }

    pub fn agent_count(&self) -> usize {
        self.by_agent.len()
    }

    pub fn entries(&self) -> &[AllowlistEntry] {
        &self.entries
    }

    pub fn contains_agent(&self, agent: &str) -> bool {
        self.by_agent.contains_key(agent)
    }
}

pub fn init(path: &Path) -> anyhow::Result<()> {
    if CATALOG.get().is_some() {
        return Ok(());
    }
    let catalog = AllowlistCatalog::from_file(path)?;
    tracing::info!(
        "MCP allowlist catalog loaded: {} agents, {} entries",
        catalog.agent_count(),
        catalog.entries.len()
    );
    let _ = CATALOG.set(catalog);
    Ok(())
}

pub fn catalog() -> &'static AllowlistCatalog {
    CATALOG.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("mcp_allowlists.toml");
        AllowlistCatalog::from_file(&path).expect("mcp_allowlists.toml missing or invalid")
    })
}

pub fn tools_for_agent(agent: &str) -> Vec<String> {
    catalog().tools_for_agent(agent)
}