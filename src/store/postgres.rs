use anyhow::Result;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::info;

use crate::config::DatabaseConfig;

pub type DbPool = PgPool;

pub async fn connect(config: &DatabaseConfig) -> Result<DbPool> {
    info!(
        max_connections = config.max_connections,
        "Connecting to database"
    );

    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.url)
        .await?;

    info!("Database connection established");

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running database
    async fn test_connect() {
        let config = DatabaseConfig {
            url: "postgres://crispr:crispr@localhost:5432/crispr_dev".to_string(),
            max_connections: 5,
        };

        let pool = connect(&config).await.unwrap();
        let row: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 1);
    }
}
