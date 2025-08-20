mod config;
mod database;
mod k_protocol;
mod listener;
mod migrations;
mod queue;
mod worker;

use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;
use tracing::{info, error, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::AppConfig;
use database::{create_pool, verify_and_setup_database};
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

    #[arg(short = 'u', long, help = "Database username")]
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

    #[arg(short = 'c', long, help = "Load configuration from a TOML file")]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing with default INFO level
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Transaction Processor");

    // Parse CLI arguments
    let args = Args::parse();
    
    // Load configuration
    let config = if let Some(ref config_path) = args.config {
        // If config file is specified, load from file and override with CLI args
        match AppConfig::from_file(config_path) {
            Ok(mut cfg) => {
                info!("Loaded configuration from: {}", config_path);
                cfg.apply_args(&args);
                cfg
            }
            Err(e) => {
                warn!("Could not load config file '{}': {}, using CLI arguments with defaults", config_path, e);
                AppConfig::from_args(&args)
            }
        }
    } else {
        // Try to load default config.toml, override with CLI args
        let config_path = std::env::current_dir()
            .map(|p| p.join("config.toml"))
            .unwrap_or_else(|_| "config.toml".into());
        
        if config_path.exists() {
            match AppConfig::from_file(config_path.to_str().unwrap_or("config.toml")) {
                Ok(mut cfg) => {
                    info!("Loaded default configuration from: {:?}", config_path);
                    cfg.apply_args(&args);
                    cfg
                }
                Err(e) => {
                    warn!("Could not load default config.toml: {}, using CLI arguments with defaults", e);
                    AppConfig::from_args(&args)
                }
            }
        } else {
            info!("No config.toml found, using CLI arguments with defaults");
            AppConfig::from_args(&args)
        }
    };
    info!("Configuration loaded: {} workers, channel: {}", 
          config.workers.count, config.processing.channel_name);
    info!("Database connection: {}:{}/{}", config.database.host, config.database.port, config.database.database);

    let db_pool = create_pool(&config).await?;
    info!("Database connection pool created with {} max connections", 
          config.database.max_connections);
    
    // Test the pool connection
    match sqlx::query("SELECT 1").fetch_one(&db_pool).await {
        Ok(_) => info!("Database pool connection test successful"),
        Err(e) => {
            error!("Database pool connection test failed: {}", e);
            return Err(e.into());
        }
    }

    // Verify and setup database requirements
    if let Err(e) = verify_and_setup_database(&db_pool).await {
        error!("Database setup failed: {}", e);
        return Err(e);
    }

    let (notification_sender, notification_receiver) = mpsc::unbounded_channel();

    let (mut notification_queue, worker_receivers) = NotificationQueue::new(
        notification_receiver,
        config.workers.count,
    );

    let notification_listener = NotificationListener::new(config.clone(), notification_sender);

    let worker_pool = WorkerPool::new(worker_receivers, db_pool, config.clone());

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
    info!("Listening for notifications on channel: {}", config.processing.channel_name);

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