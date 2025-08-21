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
use tracing::{info, error};
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

    #[arg(short = 'm', long, default_value = "10", help = "Maximum database connections")]
    db_max_connections: usize,

    #[arg(short = 'b', long, default_value = "127.0.0.1:8080", help = "Server bind address")]
    bind_address: String,

}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with default INFO level
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting K-indexer PostgreSQL webserver...");
    
    // Parse CLI arguments
    let args = Args::parse();
    
    // Load configuration from CLI arguments only
    let config = AppConfig::from_args(&args);
    
    let connection_string = config.connection_string();
    info!("Connecting to database at {}:{}", config.database.host, config.database.port);
    
    // Create database connection
    info!("Creating database connection pool with {} max connections", config.database.max_connections);
    let db_manager = match PostgresDbManager::new(&connection_string, config.database.max_connections as u32).await {
        Ok(manager) => {
            info!("Successfully connected to PostgreSQL database");
            info!("Database pool connection test successful");
            manager
        }
        Err(e) => {
            error!("Failed to connect to PostgreSQL database: {}", e);
            error!("Make sure PostgreSQL is running and the database/user exists");
            error!("Connection string (without password): postgresql://{}@{}:{}/{}", 
                   config.database.username, config.database.host, 
                   config.database.port, config.database.database);
            return Err(e.into());
        }
    };

    // Create web server
    let db_interface: Arc<dyn database_trait::DatabaseInterface> = Arc::new(db_manager);
    let web_server = WebServer::new(db_interface);

    info!("Starting web server on {}", config.server.bind_address);
    
    // Start the server
    if let Err(e) = web_server.serve(&config.server.bind_address).await {
        error!("Web server error: {}", e);
        return Err(e);
    }

    Ok(())
}