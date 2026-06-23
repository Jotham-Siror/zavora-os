//! Progressive live vs mock routing — scenarios go live as milestones ship.

pub mod tour;

use crate::events::mock;

/// Detect scenario from natural-language intent (shared with mock layer).
pub fn pick_scenario(text: &str) -> &'static str {
    mock::pick_scenario(text)
}

/// Detect action verb from second-turn intent.
pub fn pick_action(text: &str) -> Option<&'static str> {
    let t = text.to_lowercase();
    if t.contains("combine") || t.contains("merge") {
        return Some("combine");
    }
    if t.contains("fold") || t.contains("fuse") {
        return Some("fuse");
    }
    if t.contains("book") || t.contains("hold") || t.contains("reserve") {
        return Some("book");
    }
    if t.contains("reply") || t.contains("draft") {
        return Some("reply");
    }
    if t.contains("do it")
        || t.contains("handle it")
        || t.contains("take care")
        || t.contains("sort it")
        || t.contains("arrange")
        || t.contains("organize")
        || t.contains("organise")
        || t.contains("apply")
        || t.contains("read aloud")
        || t.contains("show me")
        || t.contains("reply to them")
    {
        return Some("handle");
    }
    None
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct ScenarioLiveFlags {
    pub deck: bool,
    pub morning: bool,
    pub live: bool,
    pub people: bool,
    pub week: bool,
    pub lisbon: bool,
    pub proactive: bool,
}

/// Whether an intent scenario should use the live orchestrator.
pub fn intent_is_live(scenario: &str, flags: ScenarioLiveFlags) -> bool {
    match scenario {
        "deck" => flags.deck,
        "morning" => flags.morning,
        "live" => flags.live,
        "people" => flags.people,
        "week" => flags.week,
        "lisbon" => flags.lisbon,
        "proactive" => flags.proactive,
        _ => false,
    }
}

/// Whether an action turn should use the live orchestrator.
pub fn action_is_live(action: &str, scenario: Option<&str>, deck_enabled: bool) -> bool {
    if !deck_enabled {
        return false;
    }
    match (action, scenario) {
        ("combine", Some("deck")) => true,
        _ => false,
    }
}

/// Mock conduct fuse pairs for non-live scenarios (mirrors `fusePairs` in index.html).
pub fn mock_fuse_pair(scenario: &str) -> Option<(&'static str, &'static str)> {
    match scenario {
        "morning" => Some(("Needs you", "Today")),
        "lisbon" => Some(("Flights", "Itinerary")),
        "week" => Some(("Money", "Focus")),
        "people" => Some(("Team", "Connections")),
        "live" => Some(("Headlines", "Now")),
        "proactive" => Some(("Research", "Maker")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_action_detected() {
        assert_eq!(pick_action("combine"), Some("combine"));
        assert_eq!(pick_action("merge them"), Some("combine"));
    }

    #[test]
    fn deck_combine_is_live_when_enabled() {
        assert!(action_is_live("combine", Some("deck"), true));
        assert!(!action_is_live("combine", Some("deck"), false));
        assert!(!action_is_live("combine", Some("morning"), true));
    }

    #[test]
    fn morning_intent_is_live_when_enabled() {
        let flags = ScenarioLiveFlags {
            morning: true,
            ..Default::default()
        };
        assert!(intent_is_live("morning", flags));
        assert!(!intent_is_live("morning", ScenarioLiveFlags::default()));
        assert!(intent_is_live(
            "deck",
            ScenarioLiveFlags {
                deck: true,
                ..Default::default()
            }
        ));
    }

    #[test]
    fn proactive_intent_is_live_when_enabled() {
        let flags = ScenarioLiveFlags {
            proactive: true,
            ..Default::default()
        };
        assert!(intent_is_live("proactive", flags));
        assert!(!intent_is_live("proactive", ScenarioLiveFlags::default()));
    }
}