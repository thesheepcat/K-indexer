mod config;
mod database;
mod purge_operations;

use anyhow::Result;
use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::{AppConfig, Args};
use database::create_pool;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with default INFO level
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting K-database-cleaner v{}", env!("CARGO_PKG_VERSION"));

    // Parse CLI arguments
    let args = Args::parse();

    // Load configuration from CLI arguments
    let config = AppConfig::from_args(&args);

    // Decode user pubkey from hex
    let user_pubkey = hex::decode(&config.user_pubkey).map_err(|e| {
        anyhow::anyhow!(
            "Invalid user public key hex string '{}': {}",
            config.user_pubkey,
            e
        )
    })?;

    info!(
        "Configuration: User pubkey: {}, Purge interval: {}s, Data retention: {}h",
        config.user_pubkey, config.purge_interval, config.data_retention_hours
    );
    info!(
        "Database connection: {}:{}/{}",
        config.database.host, config.database.port, config.database.database
    );

    // Create database connection pool
    let db_pool = create_pool(&config).await?;
    info!(
        "Database connection pool created with {} max connections",
        config.database.max_connections
    );

    info!("K-database-cleaner started successfully");
    info!(
        "Running purge operations every {} seconds",
        config.purge_interval
    );

    // Main purge loop
    loop {
        info!("========== Starting purge cycle ==========");
        let cycle_start = std::time::Instant::now();

        // Execute purge operations in sequence
        match purge_operations::operation_1::execute(&db_pool, &user_pubkey).await {
            Ok(_) => {}
            Err(e) => {
                error!("Purge operation 1 failed: {}", e);
                error!("Skipping remaining operations in this cycle");
                tokio::time::sleep(tokio::time::Duration::from_secs(config.purge_interval)).await;
                continue;
            }
        }

        match purge_operations::operation_2::execute(&db_pool, &user_pubkey).await {
            Ok(_) => {}
            Err(e) => {
                error!("Purge operation 2 failed: {}", e);
                error!("Skipping remaining operations in this cycle");
                tokio::time::sleep(tokio::time::Duration::from_secs(config.purge_interval)).await;
                continue;
            }
        }

        match purge_operations::operation_3::execute(
            &db_pool,
            &user_pubkey,
            config.data_retention_hours,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                error!("Purge operation 3 failed: {}", e);
                error!("Skipping remaining operations in this cycle");
                tokio::time::sleep(tokio::time::Duration::from_secs(config.purge_interval)).await;
                continue;
            }
        }

        match purge_operations::operation_4::execute(&db_pool).await {
            Ok(_) => {}
            Err(e) => {
                error!("Purge operation 4 failed: {}", e);
                error!("Skipping remaining operations in this cycle");
                tokio::time::sleep(tokio::time::Duration::from_secs(config.purge_interval)).await;
                continue;
            }
        }

        match purge_operations::operation_5::execute(&db_pool).await {
            Ok(_) => {}
            Err(e) => {
                error!("Purge operation 5 failed: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(config.purge_interval)).await;
                continue;
            }
        }

        let cycle_duration = cycle_start.elapsed();
        info!(
            "========== Purge cycle completed in {:.2}s ==========",
            cycle_duration.as_secs_f64()
        );
        info!("Next purge cycle in {} seconds", config.purge_interval);

        // Wait for the next purge interval or shutdown signal
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(config.purge_interval)) => {
                // Continue to next purge cycle
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal");
                break;
            }
        }
    }

    info!("K-database-cleaner shutting down");
    Ok(())
}
