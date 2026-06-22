use axum::{extract::Path, Json};
use serde::Serialize;

#[derive(Serialize)]
pub struct OAuthGuide {
    pub provider: String,
    pub status: &'static str,
    pub cli_command: String,
    pub docs: String,
}

/// OAuth setup guide — run MCP auth CLIs locally (M3-T7).
pub async fn oauth_guide(Path(provider): Path<String>) -> Json<OAuthGuide> {
    let provider = provider.to_lowercase();
    let (cli, docs) = match provider.as_str() {
        "google" | "gmail" => (
            "mcp-calendar auth google && mcp-email auth gmail",
            "Set GOOGLE_CALENDAR_TOKEN or run calendar auth; Gmail uses ~/.config/mcp-email tokens.",
        ),
        "microsoft" => (
            "mcp-calendar auth microsoft && mcp-email auth microsoft",
            "Set MS_GRAPH_TOKEN or run Microsoft auth flow in each MCP binary.",
        ),
        _ => (
            "mcp-calendar auth google",
            "Supported providers: google, gmail, microsoft",
        ),
    };

    Json(OAuthGuide {
        provider,
        status: "cli_auth_required",
        cli_command: cli.into(),
        docs: docs.into(),
    })
}