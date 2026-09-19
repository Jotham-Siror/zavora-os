//! Life-domain tagging (ADR-002).
//!
//! Every agent, card, SSE `card_spawn`, ledger row, memory item and pending action carries a
//! [`Domain`]. Work and Home stay logically separated; only the Mother Agent and the Balance
//! Agent read across them. `Shared` is the backward-compatible default for Phase 1 data.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Domain {
    Work,
    Home,
    #[default]
    Shared,
}

impl Domain {
    pub const ALL: [Domain; 3] = [Domain::Work, Domain::Home, Domain::Shared];

    pub fn as_str(&self) -> &'static str {
        match self {
            Domain::Work => "work",
            Domain::Home => "home",
            Domain::Shared => "shared",
        }
    }

    pub fn parse(s: &str) -> Option<Domain> {
        match s.trim().to_ascii_lowercase().as_str() {
            "work" => Some(Domain::Work),
            "home" | "personal" | "family" => Some(Domain::Home),
            "shared" | "both" => Some(Domain::Shared),
            _ => None,
        }
    }

    /// Memory-scope prefix for this domain (`work.`, `home.`, `shared.`).
    pub fn scope_prefix(&self) -> &'static str {
        match self {
            Domain::Work => "work.",
            Domain::Home => "home.",
            Domain::Shared => "shared.",
        }
    }

    /// Whether an agent living in `self` may read a memory key with the given scope prefix.
    /// Shared agents (Mother, Balance, briefing) read everything; a world reads its own
    /// scope plus `shared.`.
    pub fn may_read_scope(&self, key: &str) -> bool {
        match self {
            Domain::Shared => true,
            d => key.starts_with(d.scope_prefix()) || key.starts_with("shared."),
        }
    }

    /// Default domain of a Phase 1 scenario key (a card may override via its `domain` field).
    pub fn for_scenario(key: &str) -> Domain {
        match key {
            "deck" | "people" => Domain::Work,
            "lisbon" | "live" => Domain::Home,
            "morning" | "week" | "proactive" => Domain::Shared,
            other => Domain::parse(other).unwrap_or(Domain::Shared),
        }
    }

    /// Domain of a card: explicit `card.domain` wins, otherwise the scenario default, with a
    /// few well-known Phase 1 card agents mapped to their world.
    pub fn for_card(scenario: &str, card: &serde_json::Value) -> Domain {
        if let Some(d) = card.get("domain").and_then(|v| v.as_str()).and_then(Domain::parse) {
            return d;
        }
        match card.get("agent").and_then(|a| a.as_str()).unwrap_or("") {
            "calendar.agent" | "inbox.agent" | "work.agent" | "team.agent" | "people.agent"
            | "crm.agent" | "auto-excel" | "auto-docs" | "auto-slides" | "research.agent" => Domain::Work,
            "finance.agent" | "health.agent" | "travel.agent" | "stay.agent" | "planner.agent"
            | "markets.agent" | "maker.agent" | "scout.agent" => Domain::Home,
            "news.agent" | "live.agent" => Domain::Shared,
            _ => Domain::for_scenario(scenario),
        }
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Domain {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Domain::parse(s).ok_or_else(|| format!("unknown domain '{s}' (expected work|home|shared)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_serde_roundtrip_and_default() {
        assert_eq!(serde_json::to_string(&Domain::Work).unwrap(), "\"work\"");
        assert_eq!(serde_json::from_str::<Domain>("\"home\"").unwrap(), Domain::Home);
        assert_eq!(Domain::default(), Domain::Shared);
    }

    #[test]
    fn domain_scope_isolation() {
        assert!(Domain::Work.may_read_scope("work.preferences.email"));
        assert!(Domain::Work.may_read_scope("shared.profile.timezone"));
        assert!(!Domain::Work.may_read_scope("home.family.birthdays"));
        assert!(Domain::Shared.may_read_scope("home.family.birthdays"));
    }

    #[test]
    fn domain_for_card_prefers_explicit_then_agent_then_scenario() {
        let explicit = serde_json::json!({"agent":"calendar.agent","domain":"home"});
        assert_eq!(Domain::for_card("morning", &explicit), Domain::Home);
        let inbox = serde_json::json!({"agent":"inbox.agent"});
        assert_eq!(Domain::for_card("morning", &inbox), Domain::Work);
        let unknown = serde_json::json!({"agent":"x.agent"});
        assert_eq!(Domain::for_card("lisbon", &unknown), Domain::Home);
    }
}
