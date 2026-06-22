use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub type Db = PgPool;

pub async fn connect(database_url: &str) -> anyhow::Result<Db> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    Ok(pool)
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct User {
    pub id: uuid::Uuid,
    pub email: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub provider: String,
    pub provider_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn find_user_by_email(db: &Db, email: &str) -> anyhow::Result<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, email, name, avatar_url, provider, provider_id, created_at FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(db)
    .await?)
}

pub async fn find_user_by_id(db: &Db, id: uuid::Uuid) -> anyhow::Result<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, email, name, avatar_url, provider, provider_id, created_at FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await?)
}

pub async fn find_user_by_provider(
    db: &Db,
    provider: &str,
    provider_id: &str,
) -> anyhow::Result<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, email, name, avatar_url, provider, provider_id, created_at FROM users WHERE provider = $1 AND provider_id = $2",
    )
    .bind(provider)
    .bind(provider_id)
    .fetch_optional(db)
    .await?)
}

pub async fn create_user(
    db: &Db,
    email: &str,
    name: Option<&str>,
    provider: &str,
    provider_id: Option<&str>,
    avatar_url: Option<&str>,
) -> anyhow::Result<User> {
    Ok(sqlx::query_as::<_, User>(
        "INSERT INTO users (email, name, provider, provider_id, avatar_url) VALUES ($1, $2, $3, $4, $5) RETURNING id, email, name, avatar_url, provider, provider_id, created_at",
    )
    .bind(email)
    .bind(name)
    .bind(provider)
    .bind(provider_id)
    .bind(avatar_url)
    .fetch_one(db)
    .await?)
}