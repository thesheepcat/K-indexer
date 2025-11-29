mod config;
mod database;
mod removal_operation;

use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::{AppConfig, Args};
use database::create_pool;
use removal_operation::{execute_removal, preview_removal};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with default INFO level
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting K-content-remover v{}", env!("CARGO_PKG_VERSION"));

    // Parse CLI arguments
    let args = Args::parse();

    // Load configuration from CLI arguments
    let config = AppConfig::from_args(&args);

    // Decode target user pubkey from hex
    let target_user_pubkey = hex::decode(&config.target_user_pubkey).map_err(|e| {
        anyhow::anyhow!(
            "Invalid target user public key hex string '{}': {}",
            config.target_user_pubkey,
            e
        )
    })?;

    info!("Target user pubkey: {}", config.target_user_pubkey);
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

    // Preview what will be deleted
    info!("========== Analyzing content to remove ==========");
    let preview_stats = preview_removal(&db_pool, &target_user_pubkey).await?;

    if preview_stats.is_empty() {
        info!("No content found for user {}", config.target_user_pubkey);
        info!("Nothing to remove. Exiting.");
        return Ok(());
    }

    // If dry-run mode, exit after preview
    if config.dry_run {
        info!("========== DRY RUN MODE - No changes made ==========");
        info!("Run without --dry-run flag to actually delete the content.");
        return Ok(());
    }

    // Confirmation prompt (unless --yes flag is provided)
    if !config.skip_confirmation {
        warn!("========== CONFIRMATION REQUIRED ==========");
        warn!(
            "You are about to DELETE {} records from the database!",
            preview_stats.total()
        );
        warn!("This operation CANNOT be undone!");
        warn!("Target user: {}", config.target_user_pubkey);
        warn!("");
        warn!("Type 'DELETE' (all caps) to confirm, or anything else to cancel:");

        // Read user input from stdin
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input != "DELETE" {
            info!("Deletion cancelled by user. Exiting.");
            return Ok(());
        }
    }

    // Execute the removal
    info!("========== Executing content removal ==========");
    let removal_stats = execute_removal(&db_pool, &target_user_pubkey).await?;

    if removal_stats.total() > 0 {
        info!("========== Content removal completed successfully ==========");
        info!(
            "Removed {} total records for user {}",
            removal_stats.total(),
            config.target_user_pubkey
        );
    } else {
        warn!(
            "No records were deleted (this is unexpected - preview showed {} records)",
            preview_stats.total()
        );
    }

    Ok(())
}
