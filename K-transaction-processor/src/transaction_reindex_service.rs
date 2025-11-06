use crate::config::AppConfig;
use anyhow::Result;
use sqlx::PgPool;
use std::time::Duration;
use tracing::{error, info};

/// Reindex service that runs REINDEX CONCURRENTLY on transactions table indexes
/// every 2 hours to prevent index bloat
pub struct TransactionReindexService {
    pool: PgPool,
    interval_hours: u64,
}

impl TransactionReindexService {
    pub fn new(pool: PgPool, interval_hours: u64) -> Self {
        Self {
            pool,
            interval_hours,
        }
    }

    /// Start the reindex service loop
    pub async fn run(self) {
        info!(
            "Transaction Reindex Service started (interval: {} hours)",
            self.interval_hours
        );

        loop {
            // Wait for the configured interval
            let duration = Duration::from_secs(self.interval_hours * 3600);
            info!(
                "Waiting {} hours until next reindex operation...",
                self.interval_hours
            );
            tokio::time::sleep(duration).await;

            info!("Starting reindex operation for transactions table indexes");

            // Reindex transactions_pkey
            match self.reindex_transactions_pkey().await {
                Ok(_) => info!("Successfully reindexed transactions_pkey"),
                Err(e) => error!("Failed to reindex transactions_pkey: {}", e),
            }

            // Reindex transactions_block_time_idx
            match self.reindex_transactions_block_time_idx().await {
                Ok(_) => info!("Successfully reindexed transactions_block_time_idx"),
                Err(e) => error!("Failed to reindex transactions_block_time_idx: {}", e),
            }

            info!("Reindex operation completed");
        }
    }

    /// Reindex the primary key index on transactions table
    /// Uses REINDEX CONCURRENTLY to avoid blocking reads/writes
    async fn reindex_transactions_pkey(&self) -> Result<()> {
        info!("Reindexing transactions_pkey (this may take several minutes)...");

        // REINDEX CONCURRENTLY allows reads and writes to continue during reindex
        sqlx::query("REINDEX INDEX CONCURRENTLY transactions_pkey")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Reindex the block_time index on transactions table
    /// Uses REINDEX CONCURRENTLY to avoid blocking reads/writes
    async fn reindex_transactions_block_time_idx(&self) -> Result<()> {
        info!("Reindexing transactions_block_time_idx (this may take several minutes)...");

        // REINDEX CONCURRENTLY allows reads and writes to continue during reindex
        sqlx::query("REINDEX INDEX CONCURRENTLY transactions_block_time_idx")
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// Start the reindex service as a background task
pub async fn start_reindex_service(_config: AppConfig, pool: PgPool) {
    // Default to 2 hours interval
    let interval_hours = 2;

    let service = TransactionReindexService::new(pool, interval_hours);

    // Run the service (this will loop forever)
    service.run().await;
}
