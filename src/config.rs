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
                .unwrap_or_else(|_| "8080".into())
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