use serde::Serialize;

use crate::ambient::AmbientAgentRecord;
use crate::rails::{live::LiveSlide, people::PeopleTile};
use crate::scenarios::tour;

#[derive(Clone, Debug, Serialize)]
pub struct BgRow {
    pub label: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackgroundCard {
    pub side: String,
    pub y: u32,
    pub kind: String,
    pub intent: String,
    pub title: String,
    pub sub: String,
    pub icon: String,
    pub rows: Vec<BgRow>,
    pub activity: Vec<String>,
}

pub fn build(
    people_work: &[PeopleTile],
    people_family: &[PeopleTile],
    live_slides: &[LiveSlide],
    proactive: &[AmbientAgentRecord],
) -> Vec<BackgroundCard> {
    let mut cards = Vec::new();

    let people_rows: Vec<BgRow> = people_work
        .iter()
        .take(3)
        .map(|t| BgRow {
            label: t.1.clone(),
            detail: t.2.clone(),
        })
        .collect();
    let people_activity: Vec<String> = people_work
        .iter()
        .take(3)
        .map(|t| format!("{} — {}", t.1, t.2))
        .collect();

    cards.push(BackgroundCard {
        side: "l".into(),
        y: 14,
        kind: "people".into(),
        intent: tour::scenario_prompt("people").unwrap_or("Catch me up on people").into(),
        title: "People".into(),
        sub: "work".into(),
        icon: "👥".into(),
        rows: people_rows,
        activity: if people_activity.is_empty() {
            vec!["Catch me up on people".into()]
        } else {
            people_activity
        },
    });

    let family_rows: Vec<BgRow> = people_family
        .iter()
        .take(3)
        .map(|t| BgRow {
            label: t.1.clone(),
            detail: t.2.clone(),
        })
        .collect();
    let family_activity: Vec<String> = if family_rows.is_empty() {
        vec!["Start my day".into()]
    } else {
        family_rows
            .iter()
            .map(|r| format!("{} — {}", r.label, r.detail))
            .collect()
    };
    cards.push(BackgroundCard {
        side: "l".into(),
        y: 52,
        kind: "family".into(),
        intent: tour::scenario_prompt("morning").unwrap_or("Start my day").into(),
        title: "Family".into(),
        sub: "close".into(),
        icon: "🏡".into(),
        rows: family_rows,
        activity: family_activity,
    });

    let live_rows: Vec<BgRow> = live_slides
        .iter()
        .take(3)
        .map(|s| BgRow {
            label: s.name.clone(),
            detail: s.when.clone(),
        })
        .collect();
    let live_activity: Vec<String> = live_slides
        .iter()
        .take(3)
        .map(|s| format!("{}: {}", s.name, truncate(&s.body, 48)))
        .collect();

    cards.push(BackgroundCard {
        side: "r".into(),
        y: 12,
        kind: "live".into(),
        intent: tour::scenario_prompt("live").unwrap_or("What is happening live").into(),
        title: "Live".into(),
        sub: if live_slides.is_empty() {
            "headlines".into()
        } else {
            live_slides
                .iter()
                .take(2)
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(" · ")
        },
        icon: "📡".into(),
        rows: live_rows,
        activity: if live_activity.is_empty() {
            vec!["What is happening live".into()]
        } else {
            live_activity
        },
    });

    for (i, agent) in proactive.iter().enumerate() {
        cards.push(BackgroundCard {
            side: if i % 2 == 0 { "r" } else { "l" }.into(),
            y: 70 + (i as u32) * 9,
            kind: "proactive".into(),
            intent: tour::scenario_prompt("proactive").unwrap_or("Show me what you found").into(),
            title: agent.name.clone(),
            sub: agent.status.clone(),
            icon: agent.glyph.clone(),
            rows: vec![BgRow {
                label: agent.name.clone(),
                detail: agent.task.clone(),
            }],
            activity: vec![agent.task.clone()],
        });
    }

    cards
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}