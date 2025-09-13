mod config;
mod database;
mod k_protocol;
mod listener;
mod queue;
mod worker;

use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::AppConfig;
use database::{create_pool, KDbClient};
use listener::NotificationListener;
use queue::NotificationQueue;
use worker::WorkerPool;

#[derive(Parser, Debug)]
#[command(author, version, about = "K-indexer Transaction Processor", long_about = None)]
struct Args {
    #[arg(short = 'H', long, help = "Database host")]
    db_host: Option<String>,

    #[arg(short = 'P', long, help = "Database port")]
    db_port: Option<u16>,

    #[arg(short = 'd', long, help = "Database name")]
    db_name: Option<String>,

    #[arg(short = 'U', long, help = "Database username")]
    db_user: Option<String>,

    #[arg(short = 'p', long, help = "Database password")]
    db_password: Option<String>,

    #[arg(short = 'm', long, help = "Maximum database connections")]
    db_max_connections: Option<usize>,

    #[arg(short = 'w', long, help = "Number of worker threads")]
    workers: Option<usize>,

    #[arg(short = 'C', long, help = "PostgreSQL notification channel name")]
    channel: Option<String>,

    #[arg(short = 'r', long, help = "Number of retry attempts")]
    retry_attempts: Option<u32>,

    #[arg(short = 'D', long, help = "Retry delay in milliseconds")]
    retry_delay: Option<u64>,

    #[arg(long, help = "Initialize database (drops existing schema)")]
    initialize_db: bool,

    #[arg(short = 'u', long, help = "Enable automatic schema upgrades")]
    upgrade_db: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with default INFO level
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!(
        "Starting Transaction Processor v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Parse CLI arguments
    let args = Args::parse();

    // Load configuration from CLI arguments only
    let config = AppConfig::from_args(&args);
    info!(
        "Configuration loaded: {} workers, channel: {}",
        config.workers.count, config.processing.channel_name
    );
    info!(
        "Database connection: {}:{}/{}",
        config.database.host, config.database.port, config.database.database
    );

    let db_pool = create_pool(&config).await?;
    info!(
        "Database connection pool created with {} max connections",
        config.database.max_connections
    );

    // Test the pool connection
    match sqlx::query("SELECT 1").fetch_one(&db_pool).await {
        Ok(_) => info!("Database pool connection test successful"),
        Err(e) => {
            error!("Database pool connection test failed: {}", e);
            return Err(e.into());
        }
    }

    // Initialize database following Simply Kaspa Indexer pattern
    let database = KDbClient::new(db_pool);

    if args.initialize_db {
        info!("Initializing database");
        database.drop_schema().await.expect("Unable to drop schema");
    }
    database
        .create_schema(args.upgrade_db)
        .await
        .expect("Unable to create schema");

    let (notification_sender, notification_receiver) = mpsc::unbounded_channel();

    let (mut notification_queue, worker_receivers) =
        NotificationQueue::new(notification_receiver, config.workers.count);

    let notification_listener = NotificationListener::new(config.clone(), notification_sender);

    let worker_pool = WorkerPool::new(worker_receivers, database.pool().clone(), config.clone());

    info!("Starting all components...");

    let listener_handle = tokio::spawn(async move {
        if let Err(e) = notification_listener.start().await {
            error!("Notification listener failed: {}", e);
        }
    });

    let queue_handle = tokio::spawn(async move {
        notification_queue.start().await;
    });

    let worker_handle = tokio::spawn(async move {
        worker_pool.start().await;
    });

    info!("Transaction Processor started successfully");
    info!(
        "Listening for notifications on channel: {}",
        config.processing.channel_name
    );

    tokio::select! {
        _ = listener_handle => {
            error!("Notification listener stopped unexpectedly");
        }
        _ = queue_handle => {
            error!("Notification queue stopped unexpectedly");
        }
        _ = worker_handle => {
            error!("Worker pool stopped unexpectedly");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    info!("Transaction Processor shutting down");
    Ok(())
}
