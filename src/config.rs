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
        })
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}