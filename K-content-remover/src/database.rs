use crate::config::AppConfig;
use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing::{info, warn};

pub type DbPool = PgPool;

pub async fn create_pool(config: &AppConfig) -> Result<DbPool> {
    let connection_string = config.connection_string();

    loop {
        match PgPoolOptions::new()
            .max_connections(config.database.max_connections as u32)
            .connect(&connection_string)
            .await
        {
            Ok(pool) => {
                // Test the pool connection
                match sqlx::query("SELECT 1").fetch_one(&pool).await {
                    Ok(_) => {
                        info!("Database connection pool created and tested successfully");
                        return Ok(pool);
                    }
                    Err(e) => {
                        warn!(
                            "Database connection pool created but test query failed: {}",
                            e
                        );
                        warn!("Retrying in 10 seconds...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create database connection pool: {}", e);
                warn!("Retrying in 10 seconds...");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        }
    }
}
