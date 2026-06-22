//! Guided tour order and prompts — shared by server SSE and demo UI.

pub const ORDER: &[&str] = &[
    "morning", "people", "live", "lisbon", "week", "deck", "proactive",
];

pub fn scenario_prompt(key: &str) -> Option<&'static str> {
    match key {
        "morning" => Some("Start my day"),
        "lisbon" => Some("Plan a weekend trip to Lisbon"),
        "week" => Some("Summarize what changed this week"),
        "deck" => Some("Build me a pitch deck"),
        "people" => Some("Catch me up on people"),
        "live" => Some("What is happening live"),
        "proactive" => Some("Show me what you found"),
        _ => None,
    }
}

pub fn action_prompt(key: &str) -> Option<&'static str> {
    match key {
        "morning" => Some("handle it"),
        "lisbon" => Some("book it"),
        "week" => Some("apply it"),
        "deck" => Some("combine"),
        "people" => Some("reply to them"),
        "live" => Some("read aloud"),
        "proactive" => Some("show me"),
        _ => None,
    }
}

pub fn next_scenario(current: &str) -> Option<&'static str> {
    let idx = ORDER.iter().position(|k| *k == current)?;
    ORDER.get((idx + 1) % ORDER.len()).copied()
}