mod api_handlers;
mod config;
mod database_postgres_impl;
mod database_trait;
mod models;
mod web_server;

use clap::Parser;
use config::AppConfig;
use database_postgres_impl::PostgresDbManager;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use web_server::WebServer;

#[derive(Parser, Debug)]
#[command(author, version, about = "K-indexer PostgreSQL webserver", long_about = None)]
struct Args {
    #[arg(short = 'H', long, help = "Database host")]
    db_host: String,

    #[arg(short = 'P', long, default_value = "5432", help = "Database port")]
    db_port: u16,

    #[arg(short = 'd', long, help = "Database name")]
    db_name: String,

    #[arg(short = 'u', long, help = "Database username")]
    db_user: String,

    #[arg(short = 'p', long, help = "Database password")]
    db_password: String,

    #[arg(
        short = 'm',
        long,
        help = "Maximum database connections (defaults to worker_threads * 3)"
    )]
    db_max_connections: Option<usize>,

    #[arg(short = 'w', long, help = "Number of worker threads for Tokio runtime")]
    worker_threads: Option<usize>,

    #[arg(
        short = 't',
        long,
        default_value = "30",
        help = "Request timeout in seconds"
    )]
    request_timeout: u64,

    #[arg(
        short = 'r',
        long,
        default_value = "100",
        help = "Rate limit: requests per minute per IP"
    )]
    rate_limit: u32,

    #[arg(
        short = 'b',
        long,
        default_value = "127.0.0.1:8080",
        help = "Server bind address"
    )]
    bind_address: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI arguments first to get worker thread count
    let args = Args::parse();

    // Determine worker thread count
    let worker_threads = args.worker_threads.unwrap_or_else(|| {
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        cpu_count
    });

    // Build custom Tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()?;

    // Run the async main function
    runtime.block_on(async_main(args, worker_threads))
}

async fn async_main(args: Args, worker_threads: usize) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with default INFO level
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!(
        "Starting K-indexer PostgreSQL webserver v{}",
        env!("CARGO_PKG_VERSION")
    );
    info!("Using {} worker threads", worker_threads);
    info!("Request timeout: {}s", args.request_timeout);
    info!("Rate limit: {} requests/minute per IP", args.rate_limit);

    // Load configuration from CLI arguments only
    let config = AppConfig::from_args(&args, worker_threads);

    let connection_string = config.connection_string();
    info!(
        "Connecting to database at {}:{}",
        config.database.host, config.database.port
    );

    // Create database connection
    info!(
        "Creating database connection pool with {} max connections",
        config.database.max_connections
    );
    let db_manager =
        match PostgresDbManager::new(&connection_string, config.database.max_connections as u32)
            .await
        {
            Ok(manager) => {
                info!("Successfully connected to PostgreSQL database");
                info!("Database pool connection test successful");
                manager
            }
            Err(e) => {
                error!("Failed to connect to PostgreSQL database: {}", e);
                error!("Make sure PostgreSQL is running and the database/user exists");
                error!(
                    "Connection string (without password): postgresql://{}@{}:{}/{}",
                    config.database.username,
                    config.database.host,
                    config.database.port,
                    config.database.database
                );
                return Err(e.into());
            }
        };

    // Create web server
    let db_interface: Arc<dyn database_trait::DatabaseInterface> = Arc::new(db_manager);
    let web_server = WebServer::new(db_interface, config.server.clone()).await;

    info!("Starting web server on {}", config.server.bind_address);

    // Start the server
    if let Err(e) = web_server.serve(&config.server.bind_address).await {
        error!("Web server error: {}", e);
        return Err(e);
    }

    Ok(())
}
