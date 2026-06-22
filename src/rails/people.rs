use std::sync::Arc;

use serde::Serialize;

use crate::tools::mcp_exec;

/// People tile: [avatar, name, status, live_dot]
pub type PeopleTile = (String, String, String, bool);

#[derive(Clone, Debug, Serialize)]
pub struct PeopleRail {
    pub source: String,
    pub work: Vec<PeopleTile>,
    pub family: Vec<PeopleTile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub async fn fetch(slack: Option<Arc<dyn adk_core::Toolset>>) -> PeopleRail {
    let Some(slack) = slack else {
        return PeopleRail {
            source: "unavailable".into(),
            work: vec![],
            family: vec![],
            message: Some("Slack not connected — build mcp-slack to see your team.".into()),
        };
    };

    match fetch_slack_users(slack).await {
        Ok(work) if !work.is_empty() => PeopleRail {
            source: "slack".into(),
            work,
            family: vec![],
            message: None,
        },
        Ok(_) => PeopleRail {
            source: "slack".into(),
            work: vec![],
            family: vec![],
            message: Some("Slack connected — no active users returned.".into()),
        },
        Err(e) => PeopleRail {
            source: "slack-error".into(),
            work: vec![],
            family: vec![],
            message: Some(format!("Slack error: {e}")),
        },
    }
}

async fn fetch_slack_users(slack: Arc<dyn adk_core::Toolset>) -> anyhow::Result<Vec<PeopleTile>> {
    let resp = mcp_exec::exec_tool(slack, "list_users", serde_json::json!({ "limit": 12 }))
        .await
        .ok_or_else(|| anyhow::anyhow!("list_users unavailable"))?;
    let output = mcp_exec::tool_output_string(&resp)
        .ok_or_else(|| anyhow::anyhow!("empty slack response"))?;
    if output.starts_with("Error") {
        anyhow::bail!(output.to_string());
    }

    let users = mcp_exec::parse_array_output(output).unwrap_or_default();
    let mut tiles = Vec::new();

    for user in users.iter().take(8) {
        let name = user
            .get("real_name")
            .or_else(|| user.get("name"))
            .or_else(|| user.get("profile").and_then(|p| p.get("real_name")))
            .and_then(|v| v.as_str())
            .unwrap_or("User");
        if name == "slackbot" || name.starts_with("deleted_") {
            continue;
        }
        let initials: String = name
            .split_whitespace()
            .take(2)
            .filter_map(|w| w.chars().next())
            .collect::<String>()
            .to_uppercase();
        let avatar = if initials.len() >= 2 {
            initials
        } else {
            name.chars().take(2).collect::<String>().to_uppercase()
        };
        let status = user
            .get("presence")
            .or_else(|| user.get("status_text"))
            .and_then(|v| v.as_str())
            .unwrap_or("on Slack");
        let live = user
            .get("presence")
            .and_then(|v| v.as_str())
            .is_some_and(|p| p == "active");
        tiles.push((avatar, name.to_string(), status.to_string(), live));
    }

    Ok(tiles)
}