use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub web_dir: PathBuf,
    pub static_dir: PathBuf,
    pub audio_dir: PathBuf,
    pub business_toml: PathBuf,
    pub artifact_dir: PathBuf,
    pub mcp_worksheet_path: PathBuf,
    pub mcp_docx_path: PathBuf,
    pub mcp_slides_path: PathBuf,
    pub mcp_calendar_path: PathBuf,
    pub mcp_email_path: PathBuf,
    pub mcp_news_path: PathBuf,
    pub mcp_weather_path: PathBuf,
    pub mcp_market_data_path: PathBuf,
    pub mcp_slack_path: PathBuf,
    pub mcp_crm_path: PathBuf,
    pub mcp_banking_path: PathBuf,
    pub mcp_github_path: PathBuf,
    pub mcp_maps_path: PathBuf,
    pub mcp_real_estate_path: PathBuf,
    pub google_api_key: Option<String>,
    pub gemini_model: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        Ok(Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "9847".into())
                .parse()
                .context("PORT must be a number")?,
            web_dir: manifest_dir.join("web"),
            static_dir: manifest_dir.join("web/static"),
            audio_dir: manifest_dir.join("audio"),
            business_toml: manifest_dir.join("business.toml"),
            artifact_dir: manifest_dir
                .join(std::env::var("ARTIFACT_DIR").unwrap_or_else(|_| "./artifacts".into())),
            mcp_worksheet_path: resolve_path(
                &manifest_dir,
                "MCP_WORKSHEET_PATH",
                "../mcp-servers/worksheet-mcp/target/release/excel-mcp-server",
            ),
            mcp_docx_path: resolve_path(
                &manifest_dir,
                "MCP_DOCX_PATH",
                "../mcp-servers/docx-mcp/target/release/docx-mcp-server",
            ),
            mcp_slides_path: resolve_path(
                &manifest_dir,
                "MCP_SLIDES_PATH",
                "../mcp-servers/mcp_slides/target/release/slides-mcp-server",
            ),
            mcp_calendar_path: resolve_path(
                &manifest_dir,
                "MCP_CALENDAR_PATH",
                "../mcp-servers/mcp-calendar/target/release/mcp-calendar",
            ),
            mcp_email_path: resolve_path(
                &manifest_dir,
                "MCP_EMAIL_PATH",
                "../mcp-servers/mcp-email/target/release/mcp-email",
            ),
            mcp_news_path: resolve_path(
                &manifest_dir,
                "MCP_NEWS_PATH",
                "../mcp-servers/mcp-news/target/release/mcp-news",
            ),
            mcp_weather_path: resolve_path(
                &manifest_dir,
                "MCP_WEATHER_PATH",
                "../mcp-servers/mcp-weather/target/release/mcp-weather",
            ),
            mcp_market_data_path: resolve_path(
                &manifest_dir,
                "MCP_MARKET_DATA_PATH",
                "../mcp-servers/mcp-market-data/target/release/mcp-market-data",
            ),
            mcp_slack_path: resolve_path(
                &manifest_dir,
                "MCP_SLACK_PATH",
                "../mcp-servers/mcp-slack/target/release/mcp-slack",
            ),
            mcp_crm_path: resolve_path(
                &manifest_dir,
                "MCP_CRM_PATH",
                "../mcp-servers/mcp-crm/target/release/mcp-crm",
            ),
            mcp_banking_path: resolve_path(
                &manifest_dir,
                "MCP_BANKING_PATH",
                "../mcp-servers/mcp-banking/target/release/mcp-banking",
            ),
            mcp_github_path: resolve_path(
                &manifest_dir,
                "MCP_GITHUB_PATH",
                "../mcp-servers/mcp-github/target/release/mcp-github",
            ),
            mcp_maps_path: resolve_path(
                &manifest_dir,
                "MCP_MAPS_PATH",
                "../mcp-servers/mcp-maps/target/release/mcp-maps",
            ),
            mcp_real_estate_path: resolve_path(
                &manifest_dir,
                "MCP_REAL_ESTATE_PATH",
                "../mcp-servers/mcp-real-estate/target/release/mcp-real-estate",
            ),
            google_api_key: std::env::var("GOOGLE_API_KEY").ok().filter(|k| !k.is_empty()),
            gemini_model: std::env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-3.1-flash-lite".into()),
        })
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn deck_enabled(&self) -> bool {
        self.google_api_key.is_some()
    }

    pub fn agents_enabled(&self) -> bool {
        self.deck_enabled()
    }
}

fn resolve_path(manifest_dir: &PathBuf, env_key: &str, default: &str) -> PathBuf {
    let raw = std::env::var(env_key).unwrap_or_else(|_| default.into());
    let path = PathBuf::from(&raw);
    if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    }
}