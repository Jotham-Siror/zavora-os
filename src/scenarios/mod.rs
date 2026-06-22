//! Progressive live vs mock routing — scenarios go live as milestones ship.

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

/// Whether an intent scenario should use the live orchestrator.
pub fn intent_is_live(scenario: &str, deck_enabled: bool) -> bool {
    match scenario {
        "deck" => deck_enabled,
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
}