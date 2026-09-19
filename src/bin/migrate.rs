//! Apply pending sqlx migrations to `DATABASE_URL` without booting the server.
//!
//! Used by CI before the Postgres test groups run against the fresh service container, and by
//! deploy scripts that want the schema in place before the first request. The server still
//! runs `sqlx::migrate!` at boot, so this is a convenience, not a requirement.
//!
//! ```bash
//! DATABASE_URL=postgres://spatial_os:spatial_os@localhost:5434/spatial_os cargo run --bin migrate
//! ```

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("DATABASE_URL must be set (see .env.example)"))?;
    let pool = spatial_os::db::connect(&url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let applied: Vec<(i64, String)> =
        sqlx::query_as("SELECT version, description FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await?;
    for (version, description) in &applied {
        println!("{version:03} {description}");
    }
    println!("{} migration(s) applied", applied.len());
    Ok(())
}
