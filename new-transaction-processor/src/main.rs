mod config;
mod database;
mod k_protocol;
mod listener;
mod migrations;
mod queue;
mod worker;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{info, error, warn};
use tracing_subscriber;

use config::AppConfig;
use database::{create_pool, verify_and_setup_database};
use listener::NotificationListener;
use queue::NotificationQueue;
use worker::WorkerPool;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting Transaction Processor");

    let config_path = std::env::current_dir()
        .map(|p| p.join("config.toml"))
        .unwrap_or_else(|_| "config.toml".into());
    
    info!("Looking for config file at: {:?}", config_path);
    
    let config = AppConfig::from_file(config_path.to_str().unwrap_or("config.toml")).unwrap_or_else(|e| {
        warn!("Could not load config.toml: {}, using default configuration", e);
        AppConfig::default()
    });
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